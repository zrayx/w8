use std::error::Error;

use rzdb::{Data, Db};
use sfml::system::Vector2;

use crate::chunk::Chunk;
use crate::image::{MultiImage, IMAGES_X};
use crate::tile::Tile;
/// The first bit of the index is the sign of the coordinate - both x and y
/// idx=0 -> 0
/// idx=1 -> -1
/// idx=2 -> 1
/// idx=3 -> -2
/// idx=4 -> 2
/// idx=5 -> -3
/// idx=6 -> 3
/// positive: idx & 1 == 0, x = idx/2, idx = x*2
/// negative: idx & 1 == 1, x = -(idx/2 + 1), idx = -x*2 - 1
fn i_to_u(idx: i32) -> usize {
    if idx < 0 {
        (-(idx * 2) - 1) as usize
    } else {
        (idx * 2) as usize
    }
}

#[allow(dead_code)]
fn u_to_i(idx: usize) -> i32 {
    if idx & 1 == 0 {
        (idx / 2) as i32
    } else {
        -((idx / 2) as i32 + 1)
    }
}
pub struct Map {
    chunks: Vec<Vec<Chunk>>,
}
impl Map {
    pub fn new() -> Self {
        Map { chunks: vec![] }
    }
    pub fn get(&self, x: i32, y: i32) -> Tile {
        let (x, y) = (i_to_u(x), i_to_u(y));
        let chunk_x = x / Chunk::chunksize() as usize;
        let chunk_y = y / Chunk::chunksize() as usize;
        if chunk_x < self.chunks.len() && chunk_y < self.chunks[chunk_x].len() {
            let chunk = &self.chunks[chunk_x][chunk_y];
            let x = x % Chunk::chunksize() as usize;
            let y = y % Chunk::chunksize() as usize;
            chunk.get(x, y)
        } else {
            Tile { image_id: None }
        }
    }
    pub fn set(&mut self, x: i32, y: i32, tile: Tile) {
        let (x, y) = (i_to_u(x), i_to_u(y));
        let chunk_x = x / Chunk::chunksize() as usize;
        let chunk_y = y / Chunk::chunksize() as usize;
        let chunk = self.get_chunk_expanded_mut(chunk_x, chunk_y);
        let (x, y) = (x % Chunk::chunksize(), y % Chunk::chunksize());
        chunk.set(x, y, tile);
    }
    pub fn set_multi(&mut self, x: i32, y: i32, multi_image: MultiImage) {
        let (dx, dy) = (multi_image.size_x as i32 / 2, multi_image.size_y as i32 / 2);
        for image_id in multi_image.image_ids {
            let (image_x, image_y) = (image_id % IMAGES_X, image_id / IMAGES_X);
            let (x, y) = (
                x - dx + image_x as i32 - multi_image.min_x as i32,
                y - dy + image_y as i32 - multi_image.min_y as i32,
            );
            let tile = Tile {
                image_id: Some(image_id),
            };
            self.set(x, y, tile);
        }
    }

    fn get_chunk_expanded_mut(&mut self, chunk_x: usize, chunk_y: usize) -> &mut Chunk {
        while self.chunks.len() < chunk_x + 1 {
            self.chunks.push(vec![]);
        }
        while self.chunks[chunk_x].len() < chunk_y + 1 {
            self.chunks[chunk_x].push(Chunk::new());
        }
        &mut self.chunks[chunk_x][chunk_y]
    }
    /// Store the map in the database.
    /// Data format:
    /// chunk_x,chunk_y,y,x0,x1,x2...xn where n is Chunk::chunksize()-1
    pub fn store(&self, db: &mut Db, table_name: &str) -> Result<(), Box<dyn Error>> {
        db.create_or_replace_table(table_name)?;
        db.create_column(table_name, "chunk_x")?;
        db.create_column(table_name, "chunk_y")?;
        db.create_column(table_name, "x")?;
        for i in 0..Chunk::chunksize() {
            db.create_column(table_name, &format!("y{i}"))?;
        }

        for (x, chunk_x) in self.chunks.iter().enumerate() {
            for (y, chunk_y) in chunk_x.iter().enumerate() {
                let (x, y) = (u_to_i(x), u_to_i(y));
                chunk_y.store(db, table_name, x as usize, y as usize)?;
            }
        }
        Ok(())
    }
    pub fn parse_table(&mut self, db: &mut Db, table_name: &str) -> Result<(), Box<dyn Error>> {
        let rows = db.select_from(table_name)?;
        for row in &rows {
            if let Data::Int(chunk_x) = row.select_at(0)? {
                if let Data::Int(chunk_y) = row.select_at(1)? {
                    let (chunk_x, chunk_y) = (i_to_u(chunk_x as i32), i_to_u(chunk_y as i32));
                    let chunk = self.get_chunk_expanded_mut(chunk_x as usize, chunk_y as usize);
                    chunk.parse_row(row)?;
                } else {
                    return Err(Box::new(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid chunk_y",
                    ))));
                }
            } else {
                return Err(Box::new(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid chunk_x",
                ))));
            }
        }
        Ok(())
    }

    pub fn get_min_max(&self, tile_min_pos: &mut Vector2<i32>, tile_max_pos: &mut Vector2<i32>) {
        let len_x = self.chunks.len();
        if len_x > 0 {
            // even x: positive direction
            let max_x_chunk = u_to_i(
                (0..len_x)
                    .step_by(2)
                    .filter(|&x| !self.chunks[x].is_empty())
                    .last()
                    .unwrap_or(0),
            );
            let min_x_chunk = u_to_i(
                (1..len_x)
                    .step_by(2)
                    .filter(|&x| !self.chunks[x].is_empty())
                    .last()
                    .unwrap_or(0),
            );
            let y_min_max: Vec<(i32, i32)> = (0..len_x)
                .map(|x| {
                    let len_y = self.chunks[x].len();
                    let max_y_chunk = u_to_i(
                        (0..len_y)
                            .step_by(2)
                            .filter(|&y| !self.chunks[x][y].is_empty())
                            .last()
                            .unwrap_or(0),
                    );
                    let min_y_chunk = u_to_i(
                        (1..len_y)
                            .step_by(2)
                            .filter(|&y| !self.chunks[x][y].is_empty())
                            .last()
                            .unwrap_or(0),
                    );
                    (min_y_chunk, max_y_chunk)
                })
                .collect();
            let min_y_chunk = y_min_max.iter().map(|&(min, _)| min).min().unwrap();
            let max_y_chunk = y_min_max.iter().map(|&(_, max)| max).max().unwrap();

            tile_min_pos.x = min_x_chunk * Chunk::chunksize() as i32;
            tile_min_pos.y = min_y_chunk * Chunk::chunksize() as i32;
            tile_max_pos.x = (max_x_chunk + 1) * Chunk::chunksize() as i32 - 1;
            tile_max_pos.y = (max_y_chunk + 1) * Chunk::chunksize() as i32 - 1;
        } else {
            tile_min_pos.x = 0;
            tile_min_pos.y = 0;
            tile_max_pos.x = 0;
            tile_max_pos.y = 0;
        }
    }
}

mod test {
    #[test]
    fn test_map() {
        use super::*;
        let mut map = Map::new();
        assert_eq!(map.get(0, 0), Tile { image_id: None });
        assert_eq!(map.get(1, 0), Tile { image_id: None });
        assert_eq!(map.get(0, -1), Tile { image_id: None });
        assert_eq!(map.get(-1, 1), Tile { image_id: None });
        for y in -3..3 {
            for x in -3..3 {
                let v = (x + y + 6) as u16;
                println!("setting ({x},{y}) to {v}");
                map.set(x, y, Tile { image_id: Some(v) });
            }
        }
        for y in -3..3 {
            for x in -3..3 {
                let v_expected = (x + y + 6) as u16;
                let v_actual = map.get(x, y).image_id.unwrap();
                println!("({x},{y}) = {v_actual} (expected {v_expected})");
                assert_eq!(
                    map.get(x, y),
                    Tile {
                        image_id: Some((x + y + 6) as u16),
                    }
                );
            }
        }
    }
}
