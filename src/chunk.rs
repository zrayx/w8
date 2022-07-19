use std::error::Error;

use rzdb::{Data, Db, Row};
use sfml::system::Vector2;

use crate::tile::Tile;

pub struct Chunk {
    // Vec<Z>, Z=Vec<Y>, Y=Vec<X>
    tiles: Vec<Vec<Vec<Tile>>>,
}
impl Chunk {
    pub fn chunksize() -> usize {
        32
    }
    pub fn new() -> Self {
        Chunk { tiles: vec![] }
    }
    pub fn get(&self, x: usize, y: usize, z: usize) -> Tile {
        if z < self.tiles.len() && y < self.tiles[z].len() && x < self.tiles[z][y].len() {
            self.tiles[z][y][x]
        } else {
            Tile { image_id: None }
        }
    }
    pub fn set(&mut self, x: usize, y: usize, z: usize, tile: Tile) {
        self.expand(x, y, z);
        self.tiles[z][y][x] = tile;
    }
    fn expand(&mut self, x: usize, y: usize, z: usize) {
        while self.tiles.len() < z + 1 {
            self.tiles.push(vec![]);
        }
        while self.tiles[z].len() < y + 1 {
            self.tiles[z].push(vec![]);
        }
        while self.tiles[z][y].len() < x + 1 {
            self.tiles[z][y].push(Tile { image_id: None });
        }
    }
    pub fn get_min_max(&self, tile_min_pos: &mut Vector2<i32>, tile_max_pos: &mut Vector2<i32>) {
        let mut min_x = tile_min_pos.x;
        let mut min_y = tile_min_pos.y;
        let mut max_x = tile_max_pos.x;
        let mut max_y = tile_max_pos.y;

        for z in 0..self.tiles.len() {
            for y in 0..self.tiles[z].len() {
                for x in 0..self.tiles[z][y].len() {
                    let tile = &self.tiles[z][y][x];
                    if tile.image_id.is_some() {
                        min_x = min_x.min(x as i32);
                        min_y = min_y.min(y as i32);
                        max_x = max_x.max(x as i32);
                        max_y = max_y.max(y as i32);
                    }
                }
            }
        }
        *tile_min_pos = Vector2::new(min_x, min_y);
        *tile_max_pos = Vector2::new(max_x, max_y);
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
                if (0..Chunk::chunksize()).any(|x| self.get(x, y, z).image_id.is_some()) {
                    let mut data = vec![
                        Data::Int(chunk_x as i64),
                        Data::Int(chunk_y as i64),
                        Data::Int(chunk_z as i64),
                        Data::Int(z as i64),
                        Data::Int(y as i64),
                    ];
                    for x in 0..Chunk::chunksize() {
                        data.push(if let Some(image_id) = self.get(x, y, z).image_id {
                            Data::Int(image_id as i64)
                        } else {
                            Data::Empty
                        });
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
        if let Data::Int(z) = row.select_at(3)? {
            if let Data::Int(y) = row.select_at(4)? {
                self.expand(Chunk::chunksize() - 1, y as usize, z as usize);
                for x in 0..Chunk::chunksize() {
                    if let Data::Int(image_id) = row.select_at(x + 5)? {
                        self.set(
                            x,
                            y as usize,
                            z as usize,
                            Tile {
                                image_id: Some(image_id as u16),
                            },
                        );
                    }
                }
            } else {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid chunk data",
                )));
            }
        } else {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid chunk data",
            )));
        }
        Ok(())
    }
}
