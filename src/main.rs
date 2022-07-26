use std::fmt::Write;

use sfml::{
    graphics::{
        Color, Font, PrimitiveType, Rect, RenderStates, RenderTarget, RenderWindow, Text, Texture,
        Transform, Vertex, View,
    },
    system::{Clock, Vector2, Vector2f, Vector2i},
    window::{
        mouse::{Button, Wheel},
        ContextSettings, Event, Key, Style, VideoMode,
    },
};

use rzdb::Db;

mod chunk;
mod image;
mod map;
mod tile;

use image::{
    ImageId, MultiImage, GRASS, IMAGES_USED_X, IMAGES_USED_Y, IS_BACKGROUND, TILESIZE, WATER,
};
use map::Map;
use tile::Tile;

use crate::image::{IMAGES_CNT, IMAGES_X};

macro_rules! example_res {
    ($path:literal) => {
        concat!(env!("CARGO_MANIFEST_DIR"), "/resources/", $path)
    };
}
enum Mode {
    Paint,
    Erase,
}
struct Object {
    position: Vector2i,
    image_id: ImageId,
}
#[derive(Clone)]
enum MouseObject {
    ImageId(ImageId),
    MultiImage(MultiImage),
}

fn grid_to_win(grid_pos: Vector2i, scale: f32) -> Vector2f {
    Vector2f {
        x: grid_pos.x as f32 * TILESIZE as f32 * scale,
        y: grid_pos.y as f32 * TILESIZE as f32 * scale,
    }
}
fn win_to_grid(win_pos: Vector2f, scale: f32) -> Vector2i {
    let x = (win_pos.x / TILESIZE as f32 / scale).floor() as i32;
    let y = (win_pos.y / TILESIZE as f32 / scale).floor() as i32;
    Vector2i { x, y }
}
fn vf2i(v: Vector2f) -> Vector2i {
    Vector2i {
        x: v.x.floor() as i32,
        y: v.y.floor() as i32,
    }
}
fn vi2f(v: Vector2i) -> Vector2f {
    Vector2f {
        x: v.x as f32,
        y: v.y as f32,
    }
}
fn vu2f(v: Vector2<u32>) -> Vector2f {
    Vector2f {
        x: v.x as f32,
        y: v.y as f32,
    }
}

fn main() {
    let mut map = Map::new();
    let db_name = "w8";
    let db_dir = "~/.local/rzdb";
    let table_map = "generated_map";
    let mut db = if let Ok(mut db) = Db::load(db_name, db_dir) {
        if let Err(e) = map.parse_table(&mut db, table_map) {
            println!("{}", e);
        }
        db
    } else {
        Db::create(db_name, db_dir).unwrap()
    };
    let mut map_modified = false;
    let mut save_clock = Clock::start();

    let native_mode = VideoMode::desktop_mode();
    let mut window = RenderWindow::new(native_mode, "w8", Style::NONE, &ContextSettings::default());
    window.set_position(Vector2::new(0, 0));
    window.set_vertical_sync_enabled(true);
    let font = Font::from_file(example_res!("Qaz/Qaz.ttf")).unwrap();
    let texture = Texture::from_file(example_res!("palette.png")).unwrap();

    let multi_objects = vec![
        MultiImage::new(vec![(0, 1), (0, 2), (0, 3)]),
        MultiImage::new(vec![(1, 2), (1, 3)]),
        MultiImage::new(vec![(0, 4), (0, 5)]),
    ];
    #[allow(unused_variables)]
    let multi_ids = MultiImage::generate_multi_reverse_map(&multi_objects);
    let eraser = 3 * IMAGES_X + 3;

    let mut mode = Mode::Paint;

    let estimated_dpi = if window.size().y > 4000 { 400 } else { 300 };
    let mut scale = (estimated_dpi as f32 / 400.1 * 6.0).floor();

    let mut text_object = Text::new("", &font, 9 * scale as u32);
    // scale = 1.0;
    text_object.set_outline_color(Color::BLACK);
    text_object.set_outline_thickness(1.0);
    let mut rs = RenderStates::default();
    let mut buf = Vec::new();
    let mut current_frames_rendered = 0;
    let mut fps_clock = Clock::start();
    let mut frame_timer = Clock::start();
    let mut fps = 0;
    let mut mouse_selection = MouseObject::ImageId(0);
    let mut middle_button_start_window_xy = None;
    let mut middle_button_start_grid_xy = None;

    // map movement
    let mut dx = 94;
    let mut dy = -44;
    let mut dz = -30;
    let grid_size = win_to_grid(vu2f(window.size()), scale);
    let mut cursor_size = 1;
    let middle = grid_size / 2;
    while map.get(middle.x + dx, middle.y + dy, dz).bg.is_some() {
        dz += 1;
    }
    let mut fog = true;

    let mut clock_dx = Clock::start();
    let mut clock_dy = Clock::start();

    let (mut matrix, mut matrix_offset_y) = make_matrix(scale);

    while window.is_open() {
        // frame time for deciding if zoom can be decreased
        let frame_time = frame_timer.elapsed_time().as_milliseconds();
        frame_timer.restart();

        let mouse_pos = win_to_grid(vi2f(window.mouse_position()), scale);
        while let Some(event) = window.poll_event() {
            match event {
                Event::Closed
                | Event::KeyPressed {
                    code: Key::ESCAPE, ..
                } => window.close(),
                Event::KeyPressed { code: Key::X, .. }
                | Event::KeyPressed {
                    code: Key::DELETE, ..
                } => {
                    mode = Mode::Erase;
                    mouse_selection = MouseObject::ImageId(eraser);
                }
                Event::KeyPressed { code: Key::V, .. } => {
                    fog = !fog;
                }
                Event::KeyPressed {
                    code: Key::EQUAL, ..
                } => {
                    cursor_size_increase(&mut cursor_size);
                }
                Event::KeyPressed {
                    code: Key::HYPHEN, ..
                } => {
                    cursor_size_decrease(&mut cursor_size);
                }
                Event::MouseButtonPressed {
                    button: Button::MIDDLE,
                    ..
                } => {
                    middle_button_start_window_xy = Some(window.mouse_position());
                    middle_button_start_grid_xy = Some(Vector2i { x: dx, y: dy });
                }
                Event::MouseButtonReleased {
                    button: Button::MIDDLE,
                    ..
                } => {
                    middle_button_start_window_xy = None;
                    middle_button_start_grid_xy = None;
                }
                #[allow(unused_variables)]
                Event::MouseWheelScrolled { wheel, delta, x, y } => {
                    if wheel == Wheel::Vertical {
                        if Key::is_pressed(Key::LALT) || Key::is_pressed(Key::RALT) {
                            if delta > 0.0 {
                                cursor_size_increase(&mut cursor_size);
                            } else {
                                cursor_size_decrease(&mut cursor_size);
                            }
                        } else if Key::is_pressed(Key::LCONTROL) || Key::is_pressed(Key::RCONTROL) {
                            let device_pixels_per_tile_old = TILESIZE as f32 * scale;
                            // don't zoom out if fps would be below approx. 10
                            if delta < 0. {
                                if scale < 1.95 {
                                    if frame_time < 25 {
                                        scale /= 2.0;
                                    }
                                } else if frame_time < 50 {
                                    scale -= 1.0
                                };
                            } else if delta > 0. {
                                if scale < 1.1 {
                                    scale *= 2.0
                                } else {
                                    scale = (1.1 + scale).floor()
                                }
                            }
                            (matrix, matrix_offset_y) = make_matrix(scale);

                            // when scale is changed, we need to update the map position
                            let device_pixels_per_tile = TILESIZE as f32 * scale;
                            if device_pixels_per_tile != device_pixels_per_tile_old {
                                let mouse_pos = win_to_grid(vi2f(window.mouse_position()), scale);
                                let number_tiles_old = Vector2f {
                                    x: window.size().x as f32 / device_pixels_per_tile_old,
                                    y: window.size().y as f32 / device_pixels_per_tile_old,
                                };
                                let number_tiles = Vector2f {
                                    x: window.size().x as f32 / device_pixels_per_tile,
                                    y: window.size().y as f32 / device_pixels_per_tile,
                                };
                                let delta = number_tiles - number_tiles_old;
                                let mouse_position_relative =
                                    vi2f(window.mouse_position()) / vu2f(window.size());
                                let delta_tiles_relative =
                                    vf2i(Vector2f::new(0.5, 0.5) + delta * mouse_position_relative);
                                let (dx_old, dy_old) = (dx, dy);
                                dx -= delta_tiles_relative.x;
                                dy -= delta_tiles_relative.y;
                            }
                        } else {
                            dz -= delta as i32;
                        }
                    }
                }
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
            const F: f32 = 6.0;
            if clock_dy.elapsed_time().as_milliseconds() > 30 {
                if Key::is_pressed(Key::S) || Key::is_pressed(Key::DOWN) {
                    dy += (F / scale).max(1.0) as i32;
                    clock_dy.restart();
                } else if Key::is_pressed(Key::W) || Key::is_pressed(Key::UP) {
                    dy -= (F / scale).max(1.0) as i32;
                    clock_dy.restart();
                }
            }
            if clock_dx.elapsed_time().as_milliseconds() > 30 {
                if Key::is_pressed(Key::D) || Key::is_pressed(Key::RIGHT) {
                    dx += (F / scale).max(1.0) as i32;
                    clock_dx.restart();
                } else if Key::is_pressed(Key::A) || Key::is_pressed(Key::LEFT) {
                    dx -= (F / scale).max(1.0) as i32;
                    clock_dx.restart();
                }
            }

            if Button::LEFT.is_pressed() {
                // pick image_id from matrix
                // if mouse_pos.x < IMAGES_X as i32
                if mouse_pos.x < IMAGES_USED_X as i32
                    && mouse_pos.y >= matrix_offset_y
                    && mouse_pos.y < IMAGES_USED_Y as i32 + matrix_offset_y
                {
                    let image_id: ImageId =
                        (mouse_pos.y - matrix_offset_y) as u16 * IMAGES_X + mouse_pos.x as u16;
                    mode = if image_id == eraser {
                        Mode::Erase
                    } else {
                        Mode::Paint
                    };
                    if let Some(multi_idx) =
                        MultiImage::multi_id_from_image_id(image_id, &multi_objects)
                    {
                        mouse_selection = MouseObject::MultiImage(multi_objects[multi_idx].clone());
                    } else {
                        mouse_selection = MouseObject::ImageId(image_id);
                    }
                } else {
                    // place image_id on map or pick from map
                    let pos_x = mouse_pos.x + dx;
                    let pos_y = mouse_pos.y + dy;
                    let pos_z = dz;

                    if Key::is_pressed(Key::LALT) || Key::is_pressed(Key::RALT) {
                        // pick selected image_id from map
                        for dz in 0..10 {
                            let dz = -dz;
                            let tile = map.get(pos_x, pos_y, pos_z + dz);
                            let old_image_id = if tile.fg.is_some() { tile.fg } else { tile.bg };
                            if let Some(old_image_id) = old_image_id {
                                let old_image = if let Some(multi_idx) =
                                    MultiImage::multi_id_from_image_id(old_image_id, &multi_objects)
                                {
                                    MouseObject::MultiImage(multi_objects[multi_idx].clone())
                                } else {
                                    MouseObject::ImageId(old_image_id)
                                };
                                mouse_selection = old_image;
                                break;
                            }
                        }
                        mode = Mode::Paint;
                    } else {
                        // place image or multi-image on map
                        match mode {
                            Mode::Paint => {
                                // place image_id on map
                                match mouse_selection.clone() {
                                    MouseObject::ImageId(image_id) => {
                                        let plus_half = cursor_size / 2;
                                        let minus_half = cursor_size - plus_half - 1;
                                        for y in -minus_half..=plus_half {
                                            for x in -minus_half..=plus_half {
                                                let is_bg = IS_BACKGROUND[image_id as usize];
                                                map.set(
                                                    pos_x + x,
                                                    pos_y + y,
                                                    pos_z,
                                                    Tile {
                                                        bg: if is_bg {
                                                            Some(image_id)
                                                        } else {
                                                            Some(GRASS)
                                                        },
                                                        fg: if is_bg {
                                                            None
                                                        } else {
                                                            Some(image_id)
                                                        },
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    MouseObject::MultiImage(multi_image) => {
                                        map.set_multi_fg(pos_x, pos_y, pos_z, multi_image);
                                    }
                                }
                            }
                            Mode::Erase => {
                                // erase image_id from map
                                let plus_half = cursor_size / 2;
                                let minus_half = cursor_size - plus_half - 1;
                                for y in -minus_half..=plus_half {
                                    for x in -minus_half..=plus_half {
                                        map.set(
                                            pos_x + x,
                                            pos_y + y,
                                            pos_z,
                                            Tile { bg: None, fg: None },
                                        );
                                    }
                                }
                            }
                        }
                        save_clock.restart();
                        map_modified = true;
                    }
                }
            }
            if Button::MIDDLE.is_pressed() {
                if let (Some(start_window_xy), Some(start_grid_xy)) =
                    (middle_button_start_window_xy, middle_button_start_grid_xy)
                {
                    // mouse is at 200,200
                    // dx,dy = 3,3
                    // mouse moves to 300,300
                    // dx,dy = 3,3+(300-200,300-200)/tilesize =
                    let mouse_pos_window = window.mouse_position();
                    let window_dx = mouse_pos_window - start_window_xy;
                    let device_pixels_per_tile = TILESIZE as f32 * (scale + 0.001);
                    dx = (start_grid_xy.x as f32 - window_dx.x as f32 / device_pixels_per_tile)
                        as i32;
                    dy = (start_grid_xy.y as f32 - window_dx.y as f32 / device_pixels_per_tile)
                        as i32;
                }
            }
        }

        let mut num_sprites = matrix.len();

        // draw map
        let window_size = window.size();
        let window_vec = Vector2f {
            x: window_size.x as f32,
            y: window_size.y as f32,
        };
        let grid_size = win_to_grid(window_vec, scale);
        let tile_min_pos = Vector2i { x: dx, y: dy };
        let tile_max_pos = Vector2i {
            x: dx + grid_size.x,
            y: dy + grid_size.y,
        };

        // calculate object positions and texture coordinates
        let mut images_used = vec![];
        for pos_y in tile_min_pos.y..=tile_max_pos.y {
            for pos_x in tile_min_pos.x..=tile_max_pos.x {
                let mut visible = true;
                if fog {
                    visible = false;
                    for iz in -0..=1 {
                        for iy in -1..=1 {
                            for ix in -1..=1 {
                                let image_id = map.get(pos_x + ix, pos_y + iy, dz + iz).bg;
                                if image_id.is_none() || image_id == Some(WATER) {
                                    visible = true;
                                    break;
                                }
                            }
                        }
                    }
                }
                if visible {
                    let mut alpha = 1.0;
                    let mut image_id_bg = None;
                    let mut old_image_id_bg;
                    for pos_z_pos in 0..20 {
                        let pos_z_neg = -pos_z_pos;
                        old_image_id_bg = image_id_bg;
                        image_id_bg = map.get(pos_x, pos_y, pos_z_neg + dz).bg;
                        if image_id_bg == None || image_id_bg == Some(WATER) {
                            if pos_z_pos == 0 {
                                alpha *= 0.7;
                            } else {
                                alpha *= 0.8;
                            }
                        } else {
                            let image_id_bg = if old_image_id_bg == Some(WATER) {
                                WATER
                            } else {
                                image_id_bg.unwrap()
                            };
                            let color = Color::rgba(255, 255, 255, (alpha * 255.0) as u8);
                            push_texture_coordinates(
                                image_id_bg,
                                pos_x - dx,
                                pos_y - dy,
                                scale,
                                color,
                                &mut buf,
                            );
                            if let Some(image_id_fg) = map.get(pos_x, pos_y, pos_z_neg + dz).fg {
                                push_texture_coordinates(
                                    image_id_fg,
                                    pos_x - dx,
                                    pos_y - dy,
                                    scale,
                                    color,
                                    &mut buf,
                                );
                            }
                            num_sprites += 1;
                            while images_used.len() <= image_id_bg as usize {
                                images_used.push(0);
                            }
                            images_used[image_id_bg as usize] += 1;
                            break;
                        }
                    }
                }
            }
        }

        // matrix
        for obj in &mut matrix {
            let image_id = obj.image_id;
            let pos_x = obj.position.x;
            let pos_y = obj.position.y;
            push_texture_coordinates(image_id, pos_x, pos_y, scale, Color::WHITE, &mut buf);
        }

        // mouse
        match mouse_selection.clone() {
            MouseObject::ImageId(image_id) => {
                let plus_half = cursor_size / 2;
                let minus_half = cursor_size - plus_half - 1;
                for y in -minus_half..=plus_half {
                    for x in -minus_half..=plus_half {
                        push_texture_coordinates(
                            image_id,
                            mouse_pos.x + x,
                            mouse_pos.y + y,
                            scale,
                            Color::WHITE,
                            &mut buf,
                        );
                        num_sprites += 1;
                    }
                }
            }
            MouseObject::MultiImage(multi_image) => {
                let (dx, dy) = (multi_image.size_x as i32 / 2, multi_image.size_y as i32 / 2);
                for image_id in multi_image.image_ids {
                    let (image_x, image_y) = (image_id % IMAGES_X, image_id / IMAGES_X);
                    let (x, y) = (
                        mouse_pos.x - dx + image_x as i32 - multi_image.min_x as i32,
                        mouse_pos.y - dy + image_y as i32 - multi_image.min_y as i32,
                    );

                    push_texture_coordinates(
                        image_id,
                        x as i32,
                        y as i32,
                        scale,
                        Color::WHITE,
                        &mut buf,
                    );
                    num_sprites += 1;
                }
            }
        }

        // draw objects
        window.clear(Color::BLACK);
        rs.set_texture(Some(&texture));
        window.draw_primitives(&buf, PrimitiveType::QUADS, &rs);
        rs.set_texture(None);

        let selection_message = match mouse_selection.clone() {
            MouseObject::ImageId(image_id) => {
                format!("img:{} ", image_id)
            }
            MouseObject::MultiImage(multi_image) => {
                let mut message = "multi:".to_string();
                for image_id in multi_image.image_ids.iter() {
                    _ = write!(message, "{},", image_id);
                }
                message
            }
        };
        let mut image_message = "".to_string();
        for (image_id, count) in images_used.iter().enumerate() {
            if *count > 0 {
                _ = write!(image_message, "{}:{},", image_id, count);
            }
        }
        let ore_message = format!(
            "iron ore: {}, copper ore: {}, gold ore: {}",
            map.iron_ore_count, map.copper_ore_count, map.gold_ore_count
        );
        map.iron_ore_count = 0;
        map.copper_ore_count = 0;
        map.gold_ore_count = 0;

        let mouse_pos = win_to_grid(vi2f(window.mouse_position()), scale);
        let mouse_message = format!("mouse:{},{}", mouse_pos.x + dx, mouse_pos.y + dy);
        let message = format!(
            "{} sprites\n{} fps ({} ms per frame)\nscale: {}\nZ: {}\n{}\nfog: {}\n{}\n{}\n{}\ncursor size: {}",
            num_sprites,
            fps,
            frame_time,
            scale,
            dz,
            selection_message,
            fog,
            image_message,
            ore_message,
            mouse_message,
            cursor_size
        );
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
                panic!(" {}", err);
            }
            if let Err(err) = db.save() {
                panic!(" {}", err);
            }
            println!("{:.4} Done.", save_clock.elapsed_time().as_seconds());
            save_clock.restart();
            map_modified = false;
        }

        // calculate fps
        current_frames_rendered += 1;
        if fps_clock.elapsed_time().as_milliseconds() >= 1000 {
            fps = current_frames_rendered;
            fps_clock.restart();
            current_frames_rendered = 0;
        }
    }
}

fn cursor_size_decrease(cursor_size: &mut i32) {
    *cursor_size = match *cursor_size {
        1 => 1,
        2 => 1,
        3 => 2,
        4 => 3,
        5 => 4,
        7 => 5,
        9 => 7,
        13 => 9,
        17 => 13,
        23 => 17,
        31 => 23,
        41 => 31,
        _ => (*cursor_size * 10 / 12 - 1).max(1),
    };
}

fn cursor_size_increase(cursor_size: &mut i32) {
    *cursor_size = match *cursor_size {
        1 => 2,
        2 => 3,
        3 => 4,
        4 => 5,
        5 => 7,
        7 => 9,
        9 => 13,
        13 => 17,
        17 => 23,
        23 => 31,
        31 => 41,
        _ => *cursor_size * 12 / 10 + 1,
    };
}

fn make_matrix(scale: f32) -> (Vec<Object>, i32) {
    // matrix of objects
    let mut matrix = Vec::new();
    let matrix_offset_y = 40 / (scale - 0.1).max(1.0) as i32;
    for idx in 0..IMAGES_CNT {
        let x: i32 = (idx % IMAGES_X) as i32;
        let y: i32 = (idx / IMAGES_X) as i32 + matrix_offset_y;
        let obj = Object {
            position: Vector2i { x, y },
            image_id: idx,
        };
        matrix.push(obj);
    }
    (matrix, matrix_offset_y)
}

fn push_texture_coordinates(
    image_id: ImageId,
    pos_x: i32,
    pos_y: i32,
    scale: f32,
    color: Color,
    buf: &mut Vec<Vertex>,
) {
    let tilesize = TILESIZE as f32;
    let tex_x = f32::from(image_id % IMAGES_X) * tilesize;
    let tex_y = f32::from(image_id / IMAGES_X) * tilesize;
    let mut tf = Transform::default();
    let object_pos = grid_to_win(Vector2 { x: pos_x, y: pos_y }, scale);
    tf.translate(object_pos.x, object_pos.y);
    tf.scale_with_center(
        scale,
        scale,
        0. * scale * tilesize / 2.0,
        0. * scale * tilesize / 2.0,
    );

    buf.push(Vertex {
        color,
        position: tf.transform_point(Vector2f::new(0., 0.)),
        tex_coords: Vector2f::new(tex_x, tex_y),
    });
    buf.push(Vertex {
        color,
        position: tf.transform_point(Vector2f::new(0., tilesize)),
        tex_coords: Vector2f::new(tex_x, tex_y + tilesize),
    });
    buf.push(Vertex {
        color,
        position: tf.transform_point(Vector2f::new(tilesize, tilesize)),
        tex_coords: Vector2f::new(tex_x + tilesize, tex_y + tilesize),
    });
    buf.push(Vertex {
        color,
        position: tf.transform_point(Vector2f::new(tilesize, 0.)),
        tex_coords: Vector2f::new(tex_x + tilesize, tex_y),
    });
}
