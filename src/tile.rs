#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    pub bg: Option<u16>, // background image id, e.g. grass, dirt, stone, water, floor, etc.
    pub fg: Option<u16>, // foreground image id, e.g. tree, flower, etc.
}
