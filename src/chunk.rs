use std::error::Error;

use rzdb::{Data, Db, Row};

use crate::tile::Tile;

/// tile == None means there is no information about the tile, so it has to be generated
/// tile == Some(ImageId::None) means the tile is empty and must not be generated
pub struct Chunk {
    // Vec<Z>, Z=Vec<Y>, Y=Vec<X>
    pub tiles: Vec<Vec<Vec<Option<Tile>>>>,
}
impl Chunk {
    pub fn chunksize() -> usize {
        16
    }
    pub fn new() -> Self {
        Chunk { tiles: vec![] }
    }
    pub fn has_data(&self) -> bool {
        !self.tiles.is_empty()
    }
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<Tile> {
        if z < self.tiles.len() && y < self.tiles[z].len() && x < self.tiles[z][y].len() {
            self.tiles[z][y][x]
        } else {
            None
        }
    }
    pub fn set(&mut self, x: usize, y: usize, z: usize, tile: Tile) {
        self.expand(x, y, z);
        self.tiles[z][y][x] = Some(tile);
    }
    fn expand(&mut self, x: usize, y: usize, z: usize) {
        while self.tiles.len() < z + 1 {
            self.tiles.push(vec![]);
        }
        while self.tiles[z].len() < y + 1 {
            self.tiles[z].push(vec![]);
        }
        while self.tiles[z][y].len() < x + 1 {
            self.tiles[z][y].push(None);
        }
    }
    pub fn store(
        &self,
        db: &mut Db,
        table_name: &str,
        chunk_x: i32,
        chunk_y: i32,
        chunk_z: i32,
    ) -> Result<(), Box<dyn Error>> {
        for z in 0..Chunk::chunksize() {
            for y in 0..Chunk::chunksize() {
                // only store the data if the line is not empty
                if (0..Chunk::chunksize()).any(|x| self.get(x, y, z).is_some()) {
                    let mut data = vec![
                        Data::Int(chunk_x as i64),
                        Data::Int(chunk_y as i64),
                        Data::Int(chunk_z as i64),
                        Data::Int(z as i64),
                        Data::Int(y as i64),
                    ];
                    for x in 0..Chunk::chunksize() {
                        if let Some(tile) = self.get(x, y, z) {
                            // background
                            data.push(if let Some(image_id) = tile.bg {
                                Data::Int(image_id as i64)
                            } else {
                                Data::String("-".to_string())
                            });
                            // foreground
                            data.push(if let Some(image_id) = tile.fg {
                                Data::Int(image_id as i64)
                            } else {
                                Data::String("-".to_string())
                            });
                        } else {
                            data.push(Data::Empty);
                            data.push(Data::Empty);
                        };
                    }
                    db.insert_data(table_name, data)?;
                }
            }
        }
        Ok(())
    }
    // row format:
    // chunk_x, chunk_y, chunk_z, z, y, x0, x1, ..., x{chunksize-1}
    pub fn parse_row(&mut self, row: &Row) -> Result<(), Box<dyn Error>> {
        fn gen_error(msg: &str) -> Result<(), Box<dyn Error>> {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                msg,
            )))
        }
        let entry_to_image_id = |entry| {
            if let Data::Int(image_id) = entry {
                Some(image_id as u16)
            } else if let Data::String(s) = entry {
                if s == "-" {
                    None
                } else {
                    panic!("invalid tile entry: {}", s);
                }
            } else {
                panic!("invalid tile entry: {}", entry);
            }
        };
        if let Data::Int(z) = row.select_at(3)? {
            if let Data::Int(y) = row.select_at(4)? {
                self.expand(Chunk::chunksize() - 1, y as usize, z as usize);
                for x in 0..Chunk::chunksize() {
                    let bg = row.select_at(5 + 2 * x)?;
                    let fg = row.select_at(5 + 2 * x + 1)?;
                    match (bg, fg) {
                        (Data::Empty, Data::Empty) => {} // no entry exists
                        (bg, Data::Empty) => self.set(
                            x,
                            y as usize,
                            z as usize,
                            Tile {
                                bg: entry_to_image_id(bg),
                                fg: None,
                            },
                        ),
                        (Data::Empty, _) => unreachable!(),
                        (bg, fg) => self.set(
                            x,
                            y as usize,
                            z as usize,
                            Tile {
                                bg: entry_to_image_id(bg),
                                fg: entry_to_image_id(fg),
                            },
                        ),
                    };
                }
            } else {
                gen_error("invalid chunk data")?;
            }
        } else {
            gen_error("invalid chunk data")?;
        }
        Ok(())
    }
}
