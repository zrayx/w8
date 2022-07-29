use std::error::Error;

use rzdb::{Data, Db};

use crate::chunk::Chunk;
use crate::image::{
    MultiImage, COPPER, DIRT, GOLD, GRASS, IMAGES_X, IRON, OAK_1_1, OAK_1_1_RED, OAK_1_1_SMALL,
    PINE_1_1, STONE, WATER,
};
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
    seed: 1,
    frequency: 0.04,
    octaves: 5,
    lacunarity: 0.4,
    noise_min: NOISE_5_OCTAVES_MIN,
    noise_max: NOISE_5_OCTAVES_MAX,
    min_value: -8,
    max_value: 16,
};

const NOISE_SOIL_THICKNESS: NoiseMeta = NoiseMeta {
    id: 1,
    seed: 0,
    frequency: 0.02,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: 1,
    max_value: 5,
};

const NOISE_VEGETATION: NoiseMeta = NoiseMeta {
    id: 2,
    seed: 2,
    frequency: 0.06,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: 0,
    max_value: 50,
};

const NOISE_2D_COUNT: usize = 3;

const NOISE_IRON_ORE: NoiseMeta = NoiseMeta {
    id: 0,
    seed: 3,
    frequency: 0.06,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: -6,
    max_value: 20,
};

const NOISE_COPPER_ORE: NoiseMeta = NoiseMeta {
    id: 1,
    seed: 4,
    frequency: 0.06,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: -6,
    max_value: 20,
};

const NOISE_GOLD_ORE: NoiseMeta = NoiseMeta {
    id: 2,
    seed: 5,
    frequency: 0.16,
    octaves: 2,
    lacunarity: 0.4,
    noise_min: NOISE_2_OCTAVES_MIN,
    noise_max: NOISE_2_OCTAVES_MAX,
    min_value: -6,
    max_value: 50,
};

const NOISE_3D_COUNT: usize = 3;

struct Noise {
    data: Vec<i16>, // chunksize*chunksize values for 2d noise, chunksize*chunksize*chunksize values for 3d noise
}

pub struct Map {
    chunks_modified: Vec<Vec<Vec<Chunk>>>,
    chunks_generated: Vec<Vec<Vec<Chunk>>>,
    noise_min: f32,
    noise_max: f32,
    pub iron_ore_count: usize,
    pub copper_ore_count: usize,
    pub gold_ore_count: usize,
}
impl Map {
    pub fn new() -> Self {
        Map {
            chunks_modified: vec![],
            chunks_generated: vec![],
            noise_min: NOISE_2_OCTAVES_MIN,
            noise_max: NOISE_2_OCTAVES_MAX,
            iron_ore_count: 0,
            copper_ore_count: 0,
            gold_ore_count: 0,
        }
    }
    pub fn get(&mut self, x: i32, y: i32, z: i32) -> Tile {
        let (chunk_x, rest_x) = chunkify(x);
        let (chunk_y, rest_y) = chunkify(y);
        let (chunk_z, rest_z) = chunkify(z);
        if chunk_z < self.chunks_modified.len()
            && chunk_y < self.chunks_modified[chunk_z].len()
            && chunk_x < self.chunks_modified[chunk_z][chunk_y].len()
        {
            let chunk = &self.chunks_modified[chunk_z][chunk_y][chunk_x];
            if let Some(tile) = chunk.get(rest_x, rest_y, rest_z) {
                return tile;
            }
        }
        if chunk_z < self.chunks_modified.len()
            && chunk_y < self.chunks_modified[chunk_z].len()
            && chunk_x < self.chunks_modified[chunk_z][chunk_y].len()
        {
            let chunk = &self.chunks_modified[chunk_z][chunk_y][chunk_x];
            if let Some(tile) = chunk.get(rest_x, rest_y, rest_z) {
                return tile;
            }
        }
        self.generate_noise(chunk_x, chunk_y, chunk_z);
        self.chunks_generated[chunk_z][chunk_y][chunk_x]
            .get(rest_x, rest_y, rest_z)
            .unwrap()
    }

    // TODO: We take the old encoding and encode into the new one. Switch everything to new encoding.
    fn generate_noise(&mut self, chunk_x: usize, chunk_y: usize, chunk_z: usize) {
        let chunksize = Chunk::chunksize();
        let has_data = {
            let chunk = self.get_chunk_generated_mut(chunk_x, chunk_y, chunk_z);
            chunk.has_data()
        };
        if !has_data {
            let mut noise_2d = vec![];
            for _ in 0..NOISE_2D_COUNT {
                noise_2d.push(Noise { data: vec![] });
            }
            for (id, noise_struct) in [NOISE_TERRAIN_HEIGHT, NOISE_SOIL_THICKNESS, NOISE_VEGETATION]
                .iter()
                .enumerate()
            {
                let noise = &mut noise_2d[id];
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
                noise.data = data
                    .iter()
                    .map(|x| {
                        ((x - noise_struct.noise_min)
                            / (noise_struct.noise_max - noise_struct.noise_min)
                            * (noise_struct.max_value - noise_struct.min_value) as f32
                            + noise_struct.min_value as f32) as i16
                    })
                    .collect();
            }

            let mut noise_3d = vec![];
            for _ in 0..NOISE_3D_COUNT {
                noise_3d.push(Noise { data: vec![] });
            }
            for (id, noise_struct) in [NOISE_IRON_ORE, NOISE_COPPER_ORE, NOISE_GOLD_ORE]
                .iter()
                .enumerate()
            {
                let noise = &mut noise_3d[id];
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
                noise.data = data
                    .iter()
                    .map(|x| {
                        ((x - noise_struct.noise_min)
                            / (noise_struct.noise_max - noise_struct.noise_min)
                            * (noise_struct.max_value - noise_struct.min_value) as f32
                            + noise_struct.min_value as f32) as i16
                    })
                    .collect();
            }

            let mut tiles_z = vec![];
            for z in 0..chunksize {
                let mut tiles_y = vec![];
                for y in 0..chunksize {
                    let mut tiles_x = vec![];
                    for x in 0..chunksize {
                        let idx_2d = x + y * chunksize;

                        let terrain_height = noise_2d[NOISE_TERRAIN_HEIGHT.id].data[idx_2d];
                        let soil_thickness = noise_2d[NOISE_SOIL_THICKNESS.id].data[idx_2d];
                        let vegetation = noise_2d[NOISE_VEGETATION.id].data[idx_2d];

                        let idx_3d = x + y * chunksize + z * chunksize * chunksize;
                        let iron_ore_depth = noise_3d[NOISE_IRON_ORE.id].data[idx_3d];
                        let copper_ore_depth = noise_3d[NOISE_COPPER_ORE.id].data[idx_3d];
                        let gold_ore_depth = noise_3d[NOISE_GOLD_ORE.id].data[idx_3d];

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

                        let z_level = u_to_i(chunk_z) as i16 * chunksize as i16 + z as i16;
                        let distance = z_level as i16 - terrain_height;
                        let bg = if distance > 0 {
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
                        let fg = if bg == Some(GRASS) {
                            match vegetation {
                                0..=24 => Some(PINE_1_1),
                                25 => Some(OAK_1_1),
                                26 => Some(OAK_1_1_RED),
                                27 => Some(OAK_1_1_SMALL),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        tiles_x.push(Some(Tile { bg, fg }));
                    }
                    tiles_y.push(tiles_x);
                }
                tiles_z.push(tiles_y);
            }
            let mut chunk = self.get_chunk_generated_mut(chunk_x, chunk_y, chunk_z);
            chunk.tiles = tiles_z;
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, tile: Tile) {
        let (chunk_x, rest_x) = chunkify(x);
        let (chunk_y, rest_y) = chunkify(y);
        let (chunk_z, rest_z) = chunkify(z);
        self.get_chunk_modified_mut(chunk_x, chunk_y, chunk_z)
            .set(rest_x, rest_y, rest_z, tile);
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

    fn get_chunk_modified_mut(
        &mut self,
        chunk_x: usize,
        chunk_y: usize,
        chunk_z: usize,
    ) -> &mut Chunk {
        while self.chunks_modified.len() < chunk_z + 1 {
            self.chunks_modified.push(vec![]);
        }
        while self.chunks_modified[chunk_z].len() < chunk_y + 1 {
            self.chunks_modified[chunk_z].push(vec![]);
        }
        while self.chunks_modified[chunk_z][chunk_y].len() < chunk_x + 1 {
            self.chunks_modified[chunk_z][chunk_y].push(Chunk::new());
        }
        &mut self.chunks_modified[chunk_z][chunk_y][chunk_x]
    }

    fn get_chunk_generated_mut(
        &mut self,
        chunk_x: usize,
        chunk_y: usize,
        chunk_z: usize,
    ) -> &mut Chunk {
        while self.chunks_generated.len() < chunk_z + 1 {
            self.chunks_generated.push(vec![]);
        }
        while self.chunks_generated[chunk_z].len() < chunk_y + 1 {
            self.chunks_generated[chunk_z].push(vec![]);
        }
        while self.chunks_generated[chunk_z][chunk_y].len() < chunk_x + 1 {
            self.chunks_generated[chunk_z][chunk_y].push(Chunk::new());
        }
        &mut self.chunks_generated[chunk_z][chunk_y][chunk_x]
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

        for (z, chunk_z) in self.chunks_modified.iter().enumerate() {
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
                        let chunk = self.get_chunk_modified_mut(chunk_x, chunk_y, chunk_z);
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
