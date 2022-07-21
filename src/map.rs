use std::error::Error;

use rzdb::{Data, Db};

use crate::chunk::Chunk;
use crate::image::{MultiImage, GRASS, IMAGES_X, STONE};
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

// #[allow(dead_code)]
fn u_to_i(idx: usize) -> i32 {
    if idx & 1 == 0 {
        (idx / 2) as i32
    } else {
        -((idx / 2) as i32 + 1)
    }
}
struct Noise {
    data: Option<Vec<f32>>,
}
pub struct Map {
    chunks: Vec<Vec<Vec<Chunk>>>,
    noise: Vec<Vec<Vec<Noise>>>,
}
impl Map {
    pub fn new() -> Self {
        Map {
            chunks: vec![],
            noise: vec![],
        }
    }
    pub fn get(&mut self, x: i32, y: i32, z: i32) -> Tile {
        let (x, y, z) = (i_to_u(x), i_to_u(y), i_to_u(z));
        let chunk_x = x / Chunk::chunksize() as usize;
        let chunk_y = y / Chunk::chunksize() as usize;
        let chunk_z = z / Chunk::chunksize() as usize;
        let tile = if chunk_z < self.chunks.len()
            && chunk_y < self.chunks[chunk_z].len()
            && chunk_x < self.chunks[chunk_z][chunk_y].len()
        {
            let chunk = &self.chunks[chunk_z][chunk_y][chunk_x];
            let x = x % Chunk::chunksize() as usize;
            let y = y % Chunk::chunksize() as usize;
            let z = z % Chunk::chunksize() as usize;
            chunk.get(x, y, z)
        } else {
            Tile { image_id: None }
        };
        if tile.image_id.is_none() {
            self.get_noise(x, y, z)
        } else {
            tile
        }
    }
    fn get_noise(&mut self, x: usize, y: usize, z: usize) -> Tile {
        let (chunk_x, chunk_y, chunk_z) = (
            x / Chunk::chunksize() as usize,
            y / Chunk::chunksize() as usize,
            z / Chunk::chunksize() as usize,
        );

        while self.noise.len() <= chunk_z {
            self.noise.push(vec![]);
        }
        while self.noise[chunk_z].len() <= chunk_y {
            self.noise[chunk_z].push(vec![]);
        }
        while self.noise[chunk_z][chunk_y].len() <= chunk_x {
            self.noise[chunk_z][chunk_y].push(Noise { data: None });
        }
        let noise = &mut self.noise[chunk_z][chunk_y][chunk_x];
        if noise.data.is_none() {
            let (data, min, max) = simdnoise::NoiseBuilder::fbm_3d_offset(
                (x * Chunk::chunksize()) as f32,
                Chunk::chunksize(),
                (y * Chunk::chunksize()) as f32,
                Chunk::chunksize(),
                (z * Chunk::chunksize()) as f32,
                Chunk::chunksize(),
            )
            .with_freq(0.005)
            .with_octaves(3)
            .generate();
            noise.data = Some(data.iter().map(|x| (x - min) / (max - min)).collect());
        }
        let (nx, ny, nz) = (
            x % Chunk::chunksize() as usize,
            y % Chunk::chunksize() as usize,
            z % Chunk::chunksize() as usize,
        );
        let idx = nx
            + ny * Chunk::chunksize() as usize
            + nz * Chunk::chunksize() as usize * Chunk::chunksize() as usize;
        let value = noise.data.as_ref().unwrap()[idx];
        let height_gradient = if z > 10 {
            1.0
        } else if z > 0 {
            z as f32 / 10.0
        } else {
            0.0
        };
        let image_id = match ((value + height_gradient) * 3.0) as i32 {
            0..=2 => Some(STONE),
            3 => Some(GRASS),
            _ => None,
        };
        Tile { image_id }
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, tile: Tile) {
        let (x, y, z) = (i_to_u(x), i_to_u(y), i_to_u(z));
        let chunk_x = x / Chunk::chunksize() as usize;
        let chunk_y = y / Chunk::chunksize() as usize;
        let chunk_z = z / Chunk::chunksize() as usize;
        let chunk = self.get_chunk_expanded_mut(chunk_x, chunk_y, chunk_z);
        let (x, y, z) = (
            x % Chunk::chunksize(),
            y % Chunk::chunksize(),
            z % Chunk::chunksize(),
        );
        chunk.set(x, y, z, tile);
    }
    pub fn set_multi(&mut self, x: i32, y: i32, z: i32, multi_image: MultiImage) {
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
            self.set(x, y, z, tile);
        }
    }
    fn get_chunk_expanded_mut(
        &mut self,
        chunk_x: usize,
        chunk_y: usize,
        chunk_z: usize,
    ) -> &mut Chunk {
        while self.chunks.len() < chunk_z + 1 {
            self.chunks.push(vec![]);
        }
        while self.chunks[chunk_z].len() < chunk_y + 1 {
            self.chunks[chunk_z].push(vec![]);
        }
        while self.chunks[chunk_z][chunk_y].len() < chunk_x + 1 {
            self.chunks[chunk_z][chunk_y].push(Chunk::new());
        }
        &mut self.chunks[chunk_z][chunk_y][chunk_x]
    }
    /// Store the map in the database.
    /// Data format:
    /// chunk_x,chunk_y,chunk_z,z,y,x0,x1,x2...xn where n is Chunk::chunksize()-1
    /// see also chunk::store()
    pub fn store(&self, db: &mut Db, table_name: &str) -> Result<(), Box<dyn Error>> {
        db.create_or_replace_table(table_name)?;
        db.create_column(table_name, "chunk_x")?;
        db.create_column(table_name, "chunk_y")?;
        db.create_column(table_name, "chunk_z")?;
        db.create_column(table_name, "z")?;
        db.create_column(table_name, "y")?;
        for i in 0..Chunk::chunksize() {
            db.create_column(table_name, &format!("x{i}"))?;
        }

        for (z, chunk_z) in self.chunks.iter().enumerate() {
            for (y, chunk_y) in chunk_z.iter().enumerate() {
                for (x, chunk_x) in chunk_y.iter().enumerate() {
                    let (x, y, z) = (u_to_i(x), u_to_i(y), u_to_i(z));
                    chunk_x.store(db, table_name, x, y, z)?;
                }
            }
        }
        Ok(())
    }
    pub fn parse_table(&mut self, db: &mut Db, table_name: &str) -> Result<(), Box<dyn Error>> {
        let rows = db.select_from(table_name)?;
        let make_error = |s: &str| -> Result<(), Box<dyn Error>> {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                s,
            )))
        };
        for row in &rows {
            if let Data::Int(chunk_x) = row.select_at(0)? {
                if let Data::Int(chunk_y) = row.select_at(1)? {
                    if let Data::Int(chunk_z) = row.select_at(2)? {
                        let (chunk_x, chunk_y, chunk_z) = (
                            i_to_u(chunk_x as i32),
                            i_to_u(chunk_y as i32),
                            i_to_u(chunk_z as i32),
                        );
                        let chunk = self.get_chunk_expanded_mut(chunk_x, chunk_y, chunk_z);
                        chunk.parse_row(row)?;
                    } else {
                        return make_error("chunk_z is not an int");
                    }
                } else {
                    return make_error("chunk_y is not an int");
                }
            } else {
                return make_error("chunk_x is not an int");
            }
        }
        Ok(())
    }
}

mod test {
    #[test]
    fn test_map() {
        use super::*;
        let mut map = Map::new();
        assert_eq!(map.get(0, 0, 0), Tile { image_id: None });
        assert_eq!(map.get(1, 0, 0), Tile { image_id: None });
        assert_eq!(map.get(0, -1, 0), Tile { image_id: None });
        assert_eq!(map.get(-1, 1, 0), Tile { image_id: None });
        for z in -3..3 {
            for y in -3..3 {
                for x in -3..3 {
                    let v = (x + y + z + 9) as u16;
                    println!("setting ({x},{y},{z}) to {v}");
                    map.set(x, y, z, Tile { image_id: Some(v) });
                }
            }
        }
        for z in -3..3 {
            for y in -3..3 {
                for x in -3..3 {
                    let v_expected = (x + y + z + 9) as u16;
                    let v_actual = map.get(x, y, z).image_id.unwrap();
                    println!("({x},{y},{z}) = {v_actual} (expected {v_expected})");
                    assert_eq!(
                        map.get(x, y, z),
                        Tile {
                            image_id: Some((x + y + z + 9) as u16),
                        }
                    );
                }
            }
        }
    }
}
