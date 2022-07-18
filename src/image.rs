pub const TILESIZE: ImageId = 16;
pub const IMAGES_X: ImageId = 16;
pub const IMAGES_Y: ImageId = 16;
pub const IMAGES_CNT: ImageId = IMAGES_X * IMAGES_Y;

pub type ImageId = u16;
trait FromGrid {
    fn from_grid(x: i32, y: i32) -> Self;
}
impl FromGrid for ImageId {
    fn from_grid(x: i32, y: i32) -> Self {
        (x as ImageId + y as ImageId * IMAGES_X) as ImageId
    }
}
#[derive(Clone, Copy)]
pub struct MultiImagePart {
    pub image_id: ImageId,
    pub dx: i32,
    pub dy: i32,
}
#[derive(Clone)]
pub struct MultiImage {
    pub image_ids: Vec<ImageId>,
    pub min_x: ImageId,
    pub min_y: ImageId,
    pub size_x: ImageId,
    pub size_y: ImageId,
}
impl MultiImage {
    pub fn new(image_ids_xy: Vec<(ImageId, ImageId)>) -> Self {
        let mut image_ids = vec![];
        let mut min_x = 0;
        let mut min_y = 0;
        let mut max_x = 0;
        let mut max_y = 0;
        for (x, y) in image_ids_xy {
            assert!(x < IMAGES_X);
            assert!(y < IMAGES_Y);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            let image_id = x + y * IMAGES_X;
            image_ids.push(image_id);
        }
        let size_x = max_x - min_x + 1;
        let size_y = max_y - min_y + 1;
        MultiImage {
            image_ids,
            min_x,
            min_y,
            size_x,
            size_y,
        }
    }
    pub fn multi_id_from_image_id(image_id: ImageId, multi_array: &[MultiImage]) -> Option<usize> {
        multi_array
            .iter()
            .position(|m| m.image_ids.contains(&image_id))
    }
    pub fn generate_multi_reverse_map(multi_array: &[MultiImage]) -> Vec<Option<MultiImagePart>> {
        let mut multi_reverse_map = vec![None; IMAGES_CNT as usize];
        for (i, multi) in multi_array.iter().enumerate() {
            let mut min_x = IMAGES_X;
            let mut min_y = IMAGES_Y;
            for image_id in &multi.image_ids {
                let x = image_id % IMAGES_X;
                let y = image_id / IMAGES_X;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
            }
            for image_id in &multi.image_ids {
                let x = image_id % IMAGES_X;
                let y = image_id / IMAGES_X;
                multi_reverse_map[*image_id as usize] = Some(MultiImagePart {
                    image_id: i as ImageId,
                    dx: (x - min_x) as i32,
                    dy: (y - min_y) as i32,
                });
            }
        }
        multi_reverse_map
    }
}