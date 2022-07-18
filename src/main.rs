use rzdb::Db;
use sfml::{
    graphics::{
        Color, Font, PrimitiveType, Rect, RenderStates, RenderTarget, RenderWindow, Text, Texture,
        Transform, Vertex, View,
    },
    system::{Clock, Vector2, Vector2f, Vector2i},
    window::{mouse::Button, ContextSettings, Event, Key, Style, VideoMode},
};

mod chunk;
mod image;
mod map;
mod tile;

use image::{ImageId, MultiImage, TILESIZE};
use map::Map;
use tile::Tile;

use crate::image::{IMAGES_CNT, IMAGES_X, IMAGES_Y};

macro_rules! example_res {
    ($path:literal) => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/resources/", $path)
    };
}

const SCALE: f32 = 6.0;

struct Object {
    position: Vector2i,
    image_id: ImageId,
}
#[derive(Clone)]
enum MouseObject {
    ImageId(ImageId),
    MultiImage(MultiImage),
}

fn grid_to_win(grid_pos: Vector2i) -> Vector2f {
    Vector2f {
        x: grid_pos.x as f32 * TILESIZE as f32 * SCALE,
        y: grid_pos.y as f32 * TILESIZE as f32 * SCALE,
    }
}
fn win_to_grid(win_pos: Vector2f) -> Vector2i {
    let x = (win_pos.x / TILESIZE as f32 / SCALE).floor() as i32;
    let y = (win_pos.y / TILESIZE as f32 / SCALE).floor() as i32;
    Vector2i { x, y }
}

fn main() {
    let mut map = Map::new();
    let db_name = "w8";
    let db_dir = "~/.local/rzdb";
    let table_map = "map";
    let mut tile_min_pos = Vector2i::new(0, 0);
    let mut tile_max_pos = Vector2i::new(0, 0);
    let mut db = if let Ok(mut db) = Db::load(db_name, db_dir) {
        if let Err(e) = map.parse_table(&mut db, table_map) {
            println!("{}", e);
        } else {
            map.get_min_max(&mut tile_min_pos, &mut tile_max_pos);
        }
        db
    } else {
        Db::create(db_name, db_dir).unwrap()
    };
    let mut map_modified = false;
    let mut save_clock = Clock::start();

    let native_mode = VideoMode::desktop_mode();
    let mut window = RenderWindow::new(
        native_mode,
        "Spritemark",
        Style::NONE,
        &ContextSettings::default(),
    );
    window.set_position(Vector2::new(0, 0));
    window.set_vertical_sync_enabled(true);
    let font = Font::from_file(example_res!("sansation.ttf")).unwrap();
    let texture = Texture::from_file(example_res!("w5/palette2.png")).unwrap();
    let multi_objects = vec![
        MultiImage::new(vec![(0, 1), (0, 2), (0, 3)]),
        MultiImage::new(vec![(1, 2), (1, 3)]),
    ];
    #[allow(unused_variables)]
    let multi_ids = MultiImage::generate_multi_reverse_map(&multi_objects);
    let mut text_object = Text::new("", &font, 36);
    let mut message;
    text_object.set_outline_color(Color::BLACK);
    text_object.set_outline_thickness(1.0);
    let mut matrix = Vec::new();
    let mut rs = RenderStates::default();
    let mut buf = Vec::new();
    let mut frames_rendered = 0;
    let mut sec_clock = Clock::start();
    let mut fps = 0;
    let mut mouse_selection = MouseObject::ImageId(0);

    // map movement
    let mut dx = 0;
    let mut dy = 0;
    let mut clock_dx = Clock::start();
    let mut clock_dy = Clock::start();

    // matrix of objects
    let matrix_offset_y = 2;
    for idx in 0..IMAGES_CNT {
        let x: i32 = (idx % IMAGES_X) as i32;
        let y: i32 = (idx / IMAGES_X) as i32 + matrix_offset_y;
        let obj = Object {
            position: Vector2i { x, y },
            image_id: idx,
        };
        matrix.push(obj);
    }

    while window.is_open() {
        message = String::new();
        let mouse_pos_window = window.mouse_position();
        // let image_index: ImageId = rng.gen_range(0..IMAGES_CNT);
        let mouse_pos = win_to_grid(Vector2f {
            x: mouse_pos_window.x as f32,
            y: mouse_pos_window.y as f32,
        });
        while let Some(event) = window.poll_event() {
            match event {
                Event::Closed
                | Event::KeyPressed {
                    code: Key::ESCAPE, ..
                } => window.close(),
                Event::MouseButtonPressed {
                    button: Button::LEFT,
                    ..
                } => {}
                Event::Resized { width, height } => {
                    let window_size = Vector2i::new(width as i32, height as i32);
                    let view = View::from_rect(&Rect::new(
                        0.,
                        0.,
                        window_size.x as f32,
                        window_size.y as f32,
                    ));
                    window.set_view(&view);
                }
                _ => {}
            }
        }

        if window.has_focus() {
            if clock_dy.elapsed_time().as_milliseconds() > 30 {
                if Key::is_pressed(Key::S) || Key::is_pressed(Key::DOWN) {
                    dy += 1;
                    clock_dy.restart();
                } else if Key::is_pressed(Key::W) || Key::is_pressed(Key::UP) {
                    dy -= 1;
                    clock_dy.restart();
                }
            }
            if clock_dx.elapsed_time().as_milliseconds() > 30 {
                if Key::is_pressed(Key::D) || Key::is_pressed(Key::RIGHT) {
                    dx += 1;
                    clock_dx.restart();
                } else if Key::is_pressed(Key::A) || Key::is_pressed(Key::LEFT) {
                    dx -= 1;
                    clock_dx.restart();
                }
            }

            if Button::LEFT.is_pressed() {
                // pick image_id from matrix
                if mouse_pos.x < IMAGES_X as i32
                    && mouse_pos.y >= matrix_offset_y
                    && mouse_pos.y < IMAGES_Y as i32 + matrix_offset_y
                {
                    let image_id: ImageId =
                        (mouse_pos.y - matrix_offset_y) as u16 * IMAGES_X + mouse_pos.x as u16;
                    if let Some(multi_idx) =
                        MultiImage::multi_id_from_image_id(image_id, &multi_objects)
                    {
                        mouse_selection = MouseObject::MultiImage(multi_objects[multi_idx].clone());
                    } else {
                        mouse_selection = MouseObject::ImageId(image_id);
                    }
                } else {
                    // place image_id on map
                    let mut pos_x = mouse_pos.x + dx;
                    let mut pos_y = mouse_pos.y + dy;

                    if Key::is_pressed(Key::LALT) || Key::is_pressed(Key::RALT) {
                        // set mouse selection to image_id
                        if let Some(old_image_id) = map.get(pos_x, pos_y).image_id {
                            let old_image = if let Some(multi_idx) =
                                MultiImage::multi_id_from_image_id(old_image_id, &multi_objects)
                            {
                                MouseObject::MultiImage(multi_objects[multi_idx].clone())
                            } else {
                                MouseObject::ImageId(old_image_id)
                            };
                            mouse_selection = old_image;
                        }
                    } else {
                        match mouse_selection.clone() {
                            MouseObject::ImageId(image_id) => {
                                map.set(
                                    pos_x,
                                    pos_y,
                                    Tile {
                                        image_id: Some(image_id),
                                    },
                                );
                            }
                            MouseObject::MultiImage(multi_image) => {
                                let (more_x, more_y) =
                                    (multi_image.size_x as i32 - 1, multi_image.size_y as i32 - 1);
                                map.set_multi(pos_x, pos_y, multi_image);
                                pos_x += more_x;
                                pos_y += more_y;
                            }
                        }
                        tile_min_pos.x = pos_x.min(tile_min_pos.x);
                        tile_min_pos.y = pos_y.min(tile_min_pos.y);
                        tile_max_pos.x = pos_x.max(tile_max_pos.x);
                        tile_max_pos.y = pos_y.max(tile_max_pos.y);

                        save_clock.restart();
                        map_modified = true;
                    }
                }
            }
        }

        let mut num_sprites = matrix.len();

        // calculate object positions and texture coordinates
        // map
        for pos_y in tile_min_pos.y..=tile_max_pos.y {
            for pos_x in tile_min_pos.x..=tile_max_pos.x {
                if let Some(image_id) = map.get(pos_x + dx, pos_y + dy).image_id {
                    push_texture_coordinates(image_id, pos_x, pos_y, 1.0, &mut buf);
                    num_sprites += 1;
                }
            }
        }
        // matrix
        for obj in &mut matrix {
            let image_id = obj.image_id;
            let pos_x = obj.position.x;
            let pos_y = obj.position.y;
            push_texture_coordinates(image_id, pos_x, pos_y, 1.0, &mut buf);
        }
        // mouse
        match mouse_selection.clone() {
            MouseObject::ImageId(image_id) => {
                push_texture_coordinates(image_id, mouse_pos.x, mouse_pos.y, 1.0, &mut buf);
                num_sprites += 1;
            }
            MouseObject::MultiImage(multi_image) => {
                let (dx, dy) = (multi_image.size_x as i32 / 2, multi_image.size_y as i32 / 2);
                for image_id in multi_image.image_ids {
                    let (image_x, image_y) = (image_id % IMAGES_X, image_id / IMAGES_X);
                    let (x, y) = (
                        mouse_pos.x - dx + image_x as i32 - multi_image.min_x as i32,
                        mouse_pos.y - dy + image_y as i32 - multi_image.min_y as i32,
                    );

                    push_texture_coordinates(image_id, x as i32, y as i32, 1.0, &mut buf);
                    num_sprites += 1;
                }
            }
        }

        // draw objects
        window.clear(Color::BLACK);
        rs.set_texture(Some(&texture));
        window.draw_primitives(&buf, PrimitiveType::QUADS, &rs);
        rs.set_texture(None);

        match mouse_selection.clone() {
            MouseObject::ImageId(image_id) => {
                message += &format!("{} ", image_id);
            }
            MouseObject::MultiImage(multi_image) => {
                for image_id in multi_image.image_ids.iter() {
                    message += &format!("{},", image_id);
                }
            }
        }
        message = format!("{} sprites\n{} fps, {}", num_sprites, fps, message);
        text_object.set_string(&message);
        window.draw_text(&text_object, &rs);
        window.display();
        buf.clear();

        // save map if modified and enough time has passed
        if map_modified && save_clock.elapsed_time().as_seconds() >= 0.5 {
            println!(
                "{:.4} Saving map...",
                save_clock.elapsed_time().as_seconds()
            );
            if let Err(err) = map.store(&mut db, table_map) {
                message += &format!(" {}", err);
            }
            if let Err(err) = db.save() {
                message += &format!(" {}", err);
            }
            println!("{:.4} Done.", save_clock.elapsed_time().as_seconds());
            save_clock.restart();
            map_modified = false;
        }

        // calculate fps
        frames_rendered += 1;
        if sec_clock.elapsed_time().as_milliseconds() >= 1000 {
            fps = frames_rendered;
            sec_clock.restart();
            frames_rendered = 0;
        }
    }
}

fn push_texture_coordinates(
    image_id: ImageId,
    pos_x: i32,
    pos_y: i32,
    scale: f32,
    buf: &mut Vec<Vertex>,
) {
    let tilesize = TILESIZE as f32;
    let tex_x = f32::from(image_id % IMAGES_X) * tilesize;
    let tex_y = f32::from(image_id / IMAGES_X) * tilesize;
    let mut tf = Transform::default();
    let object_pos = grid_to_win(Vector2 { x: pos_x, y: pos_y });
    tf.translate(object_pos.x, object_pos.y);
    tf.scale_with_center(
        SCALE * scale,
        SCALE * scale,
        0. * SCALE * tilesize / 2.0,
        0. * SCALE * tilesize / 2.0,
    );
    buf.push(Vertex {
        color: Color::WHITE,
        position: tf.transform_point(Vector2f::new(0., 0.)),
        tex_coords: Vector2f::new(tex_x, tex_y),
    });
    buf.push(Vertex {
        color: Color::WHITE,
        position: tf.transform_point(Vector2f::new(0., tilesize)),
        tex_coords: Vector2f::new(tex_x, tex_y + tilesize),
    });
    buf.push(Vertex {
        color: Color::WHITE,
        position: tf.transform_point(Vector2f::new(tilesize, tilesize)),
        tex_coords: Vector2f::new(tex_x + tilesize, tex_y + tilesize),
    });
    buf.push(Vertex {
        color: Color::WHITE,
        position: tf.transform_point(Vector2f::new(tilesize, 0.)),
        tex_coords: Vector2f::new(tex_x + tilesize, tex_y),
    });
}
