use std::error::Error;

use rzdb::{Data, Db, Row};

use crate::tile::Tile;

pub struct Chunk {
    tiles: Vec<Vec<Tile>>,
}
impl Chunk {
    pub fn chunksize() -> usize {
        32
    }
    pub fn new() -> Self {
        Chunk { tiles: vec![] }
    }
    pub fn get(&self, x: usize, y: usize) -> Tile {
        if x < self.tiles.len() && y < self.tiles[x].len() {
            self.tiles[x][y]
        } else {
            Tile { image_id: None }
        }
    }
    pub fn set(&mut self, x: usize, y: usize, tile: Tile) {
        self.expand(x, y);
        self.tiles[x][y] = tile;
    }
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
    fn expand(&mut self, x: usize, y: usize) {
        while self.tiles.len() < x + 1 {
            self.tiles.push(vec![]);
        }
        while self.tiles[x].len() < y + 1 {
            self.tiles[x].push(Tile { image_id: None });
        }
    }
    pub fn store(
        &self,
        db: &mut Db,
        table_name: &str,
        chunk_x: usize,
        chunk_y: usize,
    ) -> Result<(), Box<dyn Error>> {
        for y in 0..Chunk::chunksize() {
            if (0..Chunk::chunksize()).any(|x| self.get(x, y).image_id.is_some()) {
                let mut data = vec![
                    Data::Int(chunk_x as i64),
                    Data::Int(chunk_y as i64),
                    Data::Int(y as i64),
                ];
                // test if the chunk is empty
                for x in 0..Chunk::chunksize() {
                    data.push(if let Some(image_id) = self.get(x, y).image_id {
                        Data::Int(image_id as i64)
                    } else {
                        Data::Empty
                    });
                }
                db.insert_data(table_name, data)?;
            }
        }
        Ok(())
    }
    pub fn parse_row(&mut self, row: &Row) -> Result<(), Box<dyn Error>> {
        if let Data::Int(y) = row.select_at(2)? {
            self.expand(Chunk::chunksize() - 1, y as usize);
            for x in 0..Chunk::chunksize() {
                if let Data::Int(image_id) = row.select_at(x + 3)? {
                    self.set(
                        x,
                        y as usize,
                        Tile {
                            image_id: Some(image_id as u16),
                        },
                    );
                }
            }
        } else {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Chunk::parse_row: invalid data at x={} y={}", 0, 0,),
            )));
        }
        Ok(())
    }
}
