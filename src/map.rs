use std::error::Error;

use rzdb::{Data, Db};

use crate::chunk::Chunk;
use crate::image::{MultiImage, COPPER, DIRT, GOLD, GRASS, IMAGES_X, IRON, STONE, WATER};
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

struct NoiseMeta {
    id: usize,
    frequency: f32,
    octaves: u8, // changes noise_min/noise_max
    lacunarity: f32,
    noise_min: f32,
    noise_max: f32,
    min_value: i16, // quality of values near min_value and max_value depend on the accuracy
    max_value: i16, // noise_min and noise_max
    seed: i32,
}

const NOISE_2_OCTAVES_MIN: f32 = -0.0911;
const NOISE_2_OCTAVES_MAX: f32 = 0.0911;
const NOISE_5_OCTAVES_MIN: f32 = -0.66;
const NOISE_5_OCTAVES_MAX: f32 = 0.66;

const NOISE_TERRAIN_HEIGHT: NoiseMeta = NoiseMeta {
    id: 0,
    frequency: 0.04,
    octaves: 5,
    lacunarity: 0.4,
    noise_min: NOISE_5_OCTAVES_MIN,
    noise_max: NOISE_5_OCTAVES_MAX,
    min_value: -8,
    max_value: 16,
    seed: 1,
};
const NOISE_SOIL_THICKNESS: NoiseMeta = NoiseMeta {
    id: 1,
    frequency: 0.02,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: 1,
    max_value: 5,
    seed: 0,
};
const NOISE_2D_COUNT: usize = 2;

const NOISE_IRON_ORE: NoiseMeta = NoiseMeta {
    id: 0,
    frequency: 0.06,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: -6,
    max_value: 20,
    seed: 3,
};

const NOISE_COPPER_ORE: NoiseMeta = NoiseMeta {
    id: 1,
    frequency: 0.06,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: -6,
    max_value: 20,
    seed: 4,
};

const NOISE_GOLD_ORE: NoiseMeta = NoiseMeta {
    id: 2,
    frequency: 0.16,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: -6,
    max_value: 50,
    seed: 5,
};

const NOISE_3D_COUNT: usize = 3;

struct Noise {
    data: Option<Vec<i16>>, // "2d" array of chunksize*chunksize values
}

pub struct Map {
    chunks: Vec<Vec<Vec<Chunk>>>,
    noise_2d: Vec<Vec<Vec<Noise>>>, // array of array[][] of chunks
    noise_3d: Vec<Vec<Vec<Vec<Noise>>>>, // array of array[][][] of chunks
    noise_min: f32,
    noise_max: f32,
    pub iron_ore_count: usize,
    pub copper_ore_count: usize,
    pub gold_ore_count: usize,
}
impl Map {
    pub fn new() -> Self {
        let mut noise_2d = vec![];
        for _ in 0..NOISE_2D_COUNT {
            noise_2d.push(vec![]);
        }
        let mut noise_3d = vec![];
        for _ in 0..NOISE_3D_COUNT {
            noise_3d.push(vec![]);
        }
        Map {
            chunks: vec![],
            noise_2d,
            noise_3d,
            noise_min: NOISE_2_OCTAVES_MIN,
            noise_max: NOISE_2_OCTAVES_MAX,
            iron_ore_count: 0,
            copper_ore_count: 0,
            gold_ore_count: 0,
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
                self.get_from_noise(encoded_x, encoded_y, encoded_z)
            }
        } else {
            self.get_from_noise(encoded_x, encoded_y, encoded_z)
        }
    }

    // TODO: We take the old encoding and encode into the new one. Switch everything to new encoding.
    fn get_from_noise(&mut self, encoded_x: usize, encoded_y: usize, encoded_z: usize) -> Tile {
        let chunksize = Chunk::chunksize();
        let (decoded_x, decoded_y, z_level) =
            (u_to_i(encoded_x), u_to_i(encoded_y), u_to_i(encoded_z));
        let (chunk_x, rest_x) = chunkify(decoded_x);
        let (chunk_y, rest_y) = chunkify(decoded_y);
        let (chunk_z, rest_z) = chunkify(z_level);

        for id in 0..NOISE_2D_COUNT {
            let noise_struct = match id {
                0 => &NOISE_TERRAIN_HEIGHT,
                1 => &NOISE_SOIL_THICKNESS,
                _ => unreachable!(),
            };
            while self.noise_2d[id].len() <= chunk_y {
                self.noise_2d[id].push(vec![]);
            }
            while self.noise_2d[id][chunk_y].len() <= chunk_x {
                self.noise_2d[id][chunk_y].push(Noise { data: None });
            }
            let noise = &mut self.noise_2d[id][chunk_y][chunk_x];
            if noise.data.is_none() {
                let (data, min, max) = simdnoise::NoiseBuilder::fbm_2d_offset(
                    (u_to_i(chunk_x) * chunksize as i32) as f32,
                    chunksize,
                    (u_to_i(chunk_y) * chunksize as i32) as f32,
                    chunksize,
                )
                .with_freq(noise_struct.frequency)
                .with_octaves(noise_struct.octaves)
                .with_lacunarity(noise_struct.lacunarity)
                .with_seed(noise_struct.seed)
                .generate();
                if min < noise_struct.noise_min && id > 0 && min < self.noise_min {
                    self.noise_min = self.noise_min.min(min);
                    println!("new noise_2d[{}] min: {}", id, min);
                }
                if max > noise_struct.noise_max && id > 0 && max > self.noise_max {
                    self.noise_max = self.noise_max.max(max);
                    println!("new noise_2d[{}] max: {}", id, max);
                }
                noise.data = Some(
                    data.iter()
                        .map(|x| {
                            ((x - noise_struct.noise_min)
                                / (noise_struct.noise_max - noise_struct.noise_min)
                                * (noise_struct.max_value - noise_struct.min_value) as f32
                                + noise_struct.min_value as f32) as i16
                        })
                        .collect(),
                );
            }
        }

        for id in 0..NOISE_3D_COUNT {
            let noise_struct = match id {
                0 => &NOISE_IRON_ORE,
                1 => &NOISE_COPPER_ORE,
                2 => &NOISE_GOLD_ORE,
                _ => unreachable!(),
            };
            while self.noise_3d[id].len() <= chunk_z {
                self.noise_3d[id].push(vec![]);
            }
            while self.noise_3d[id][chunk_z].len() <= chunk_y {
                self.noise_3d[id][chunk_z].push(vec![]);
            }
            while self.noise_3d[id][chunk_z][chunk_y].len() <= chunk_x {
                self.noise_3d[id][chunk_z][chunk_y].push(Noise { data: None });
            }
            let noise = &mut self.noise_3d[id][chunk_z][chunk_y][chunk_x];
            if noise.data.is_none() {
                let (data, min, max) = simdnoise::NoiseBuilder::fbm_3d_offset(
                    (u_to_i(chunk_x) * chunksize as i32) as f32,
                    chunksize,
                    (u_to_i(chunk_y) * chunksize as i32) as f32,
                    chunksize,
                    (u_to_i(chunk_z) * chunksize as i32) as f32,
                    chunksize,
                )
                .with_freq(noise_struct.frequency)
                .with_octaves(noise_struct.octaves)
                .with_lacunarity(noise_struct.lacunarity)
                .with_seed(noise_struct.seed)
                .generate();
                if min < noise_struct.noise_min && id > 0 && min < self.noise_min {
                    self.noise_min = self.noise_min.min(min);
                    println!("new noise_3d[{}] min: {}", id, min);
                }
                if max > noise_struct.noise_max && id > 0 && max > self.noise_max {
                    self.noise_max = self.noise_max.max(max);
                    println!("new noise_3d[{}] max: {}", id, max);
                }
                noise.data = Some(
                    data.iter()
                        .map(|x| {
                            ((x - noise_struct.noise_min)
                                / (noise_struct.noise_max - noise_struct.noise_min)
                                * (noise_struct.max_value - noise_struct.min_value) as f32
                                + noise_struct.min_value as f32) as i16
                        })
                        .collect(),
                );
            }
        }

        let idx_2d = rest_x + rest_y * chunksize;

        let terrain_height = &self.noise_2d[NOISE_TERRAIN_HEIGHT.id][chunk_y][chunk_x];
        let terrain_height = terrain_height.data.as_ref().unwrap()[idx_2d];

        let soil_thickness = &self.noise_2d[NOISE_SOIL_THICKNESS.id][chunk_y][chunk_x];
        let soil_thickness = soil_thickness.data.as_ref().unwrap()[idx_2d];

        let idx_3d = rest_x + rest_y * chunksize + rest_z * chunksize * chunksize;
        let iron_ore_depth = &self.noise_3d[NOISE_IRON_ORE.id][chunk_z][chunk_y][chunk_x];
        let iron_ore_depth = iron_ore_depth.data.as_ref().unwrap()[idx_3d];
        let copper_ore_depth = &self.noise_3d[NOISE_COPPER_ORE.id][chunk_z][chunk_y][chunk_x];
        let copper_ore_depth = copper_ore_depth.data.as_ref().unwrap()[idx_3d];
        let gold_ore_depth = &self.noise_3d[NOISE_GOLD_ORE.id][chunk_z][chunk_y][chunk_x];
        let gold_ore_depth = gold_ore_depth.data.as_ref().unwrap()[idx_3d];

        let mut ore_kind = STONE;
        let mut chooser = |value, ore_type| {
            if value < 0 {
                ore_kind = ore_type;
            }
        };
        // latter overwrites former
        chooser(copper_ore_depth, COPPER);
        chooser(gold_ore_depth, GOLD);
        chooser(iron_ore_depth, IRON);
        match ore_kind {
            IRON => self.iron_ore_count += 1,
            COPPER => self.copper_ore_count += 1,
            GOLD => self.gold_ore_count += 1,
            _ => (),
        }

        let distance = z_level as i16 - terrain_height;
        let image_id = if distance > 0 {
            if terrain_height <= 0 && z_level <= 0 {
                Some(WATER)
            } else {
                None
            }
        } else if distance == 0 {
            if terrain_height >= 0 {
                Some(GRASS)
            } else {
                Some(DIRT)
            }
        } else if distance < 0 && distance >= -soil_thickness {
            Some(DIRT)
        } else {
            Some(ore_kind)
        };
        Tile {
            bg: image_id,
            fg: None,
        }
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
    pub fn set_multi_fg(&mut self, x: i32, y: i32, z: i32, multi_image: MultiImage) {
        let (dx, dy) = (multi_image.size_x as i32 / 2, multi_image.size_y as i32 / 2);
        for image_id in multi_image.image_ids {
            let (image_x, image_y) = (image_id % IMAGES_X, image_id / IMAGES_X);
            let (x, y) = (
                x - dx + image_x as i32 - multi_image.min_x as i32,
                y - dy + image_y as i32 - multi_image.min_y as i32,
            );
            let tile = Tile {
                bg: Some(GRASS),
                fg: Some(image_id),
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
            db.create_column(table_name, &format!("bg{i}"))?;
            db.create_column(table_name, &format!("fg{i}"))?;
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
