use std::cmp::Ordering;
use std::error::Error;

use rzdb::{Data, Db};

use crate::chunk::Chunk;
use crate::image::{ImageId, MultiImage, DIRT, GRASS, IMAGES_X, STONE, WATER};
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

fn chunkify(i: i32) -> (usize, usize) {
    let cs = Chunk::chunksize() as i32;
    let (chunk, rest) = if i < 0 {
        ((i - cs + 1) / cs, (i + 1) % cs + cs - 1)
    } else {
        (i / cs, i % cs)
    };
    (i_to_u(chunk), rest as usize)
}

struct Noise {
    data: Option<Vec<f32>>,
}
pub struct Map {
    chunks: Vec<Vec<Vec<Chunk>>>,
    noise_height: Vec<Vec<Noise>>,
    noise_min: f32,
    noise_max: f32,
}
impl Map {
    pub fn new() -> Self {
        Map {
            chunks: vec![],
            noise_height: vec![],
            noise_min: -0.66, // these values have to be adjusted if new min/max values are found
            noise_max: 0.66,  // current values found are +/-0.62
        }
    }
    pub fn get(&mut self, x: i32, y: i32, z: i32) -> Tile {
        let (encoded_x, encoded_y, encoded_z) = (i_to_u(x), i_to_u(y), i_to_u(z));
        let chunk_x = encoded_x / Chunk::chunksize() as usize;
        let chunk_y = encoded_y / Chunk::chunksize() as usize;
        let chunk_z = encoded_z / Chunk::chunksize() as usize;
        if chunk_z < self.chunks.len()
            && chunk_y < self.chunks[chunk_z].len()
            && chunk_x < self.chunks[chunk_z][chunk_y].len()
        {
            let chunk = &self.chunks[chunk_z][chunk_y][chunk_x];
            let x = encoded_x % Chunk::chunksize() as usize;
            let y = encoded_y % Chunk::chunksize() as usize;
            let z = encoded_z % Chunk::chunksize() as usize;
            if let Some(tile) = chunk.get(x, y, z) {
                tile
            } else {
                self.get_noise(encoded_x, encoded_y, encoded_z)
            }
        } else {
            self.get_noise(encoded_x, encoded_y, encoded_z)
        }
    }

    // TODO: We take the old encoding and encode into the new one. Switch everything to new encoding.
    fn get_noise(&mut self, encoded_x: usize, encoded_y: usize, encoded_z: usize) -> Tile {
        let chunksize = Chunk::chunksize();
        let (decoded_x, decoded_y, z_level) =
            (u_to_i(encoded_x), u_to_i(encoded_y), u_to_i(encoded_z));
        let ((chunk_x, rest_x), (chunk_y, rest_y)) = (chunkify(decoded_x), chunkify(decoded_y));

        while self.noise_height.len() <= chunk_y {
            self.noise_height.push(vec![]);
        }
        while self.noise_height[chunk_y].len() <= chunk_x {
            self.noise_height[chunk_y].push(Noise { data: None });
        }
        let noise = &mut self.noise_height[chunk_y][chunk_x];
        if noise.data.is_none() {
            let (data, min, max) = simdnoise::NoiseBuilder::fbm_2d_offset(
                (u_to_i(chunk_x) * chunksize as i32) as f32,
                chunksize,
                (u_to_i(chunk_y) * chunksize as i32) as f32,
                chunksize,
            )
            .with_freq(0.04)
            .with_octaves(5)
            .generate();
            if min < self.noise_min {
                self.noise_min = min;
                println!("new noise min: {}", self.noise_min);
            }
            if max > self.noise_max {
                self.noise_max = max;
                println!("new noise max: {}", self.noise_max);
            }
            noise.data = Some(
                data.iter()
                    .map(|x| (x - self.noise_min) / (self.noise_max - self.noise_min))
                    .collect(),
            );
        }
        let idx = rest_x + rest_y * chunksize;
        let value = noise.data.as_ref().unwrap()[idx];
        let air_level = (value * 9.0 - 4.0) as i32;
        let maybe_water = |image_id: ImageId| {
            if air_level < 0 {
                Some(WATER)
            } else {
                Some(image_id)
            }
        };

        let image_id = match z_level.cmp(&air_level) {
            Ordering::Less => {
                if z_level == air_level - 1 {
                    maybe_water(DIRT)
                } else {
                    maybe_water(STONE)
                }
            }
            Ordering::Equal => maybe_water(GRASS),
            Ordering::Greater => None,
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
