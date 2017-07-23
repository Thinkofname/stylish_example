
use stylish::*;
use stylish_webrender::Info;

pub struct Center;

impl LayoutEngine<Info> for Center {
    fn pre_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        parent: &RenderObject<Info>
    ) {
        obj.draw_rect = Rect {
            x: 0, y: 0,
            .. parent.draw_rect
        };
        obj.max_size = parent.max_size;
        if let Some(width) = obj.get_value("width") {
            obj.draw_rect.width = width;
            obj.max_size.0 = Some(width);
        }
        if let Some(height) = obj.get_value("height") {
            obj.draw_rect.height = height;
            obj.max_size.1 = Some(height);
        }
    }
    fn post_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        parent: &RenderObject<Info>
    ) {
        if obj.get_value::<bool>("align_width").unwrap_or(true) {
            obj.draw_rect.x = (parent.draw_rect.width / 2) - (obj.draw_rect.width / 2);
        } else if let Some(x) = obj.get_value::<i32>("x") {
            obj.draw_rect.x = x;
        }
        if obj.get_value::<bool>("align_height").unwrap_or(true) {
            obj.draw_rect.y = (parent.draw_rect.height / 2) - (obj.draw_rect.height / 2);
        } else if let Some(y) = obj.get_value::<i32>("y") {
            obj.draw_rect.y = y;
        }
    }
    fn finalize_layout(
        &mut self,
        _obj: &mut RenderObject<Info>,
        _children: Vec<&mut RenderObject<Info>>
    ) {

    }
}


pub struct Padded {
    padding: i32,
}

impl Padded {
    pub fn new(obj: &RenderObject<Info>) -> Padded {
        Padded {
            padding: obj.get_value("padding").unwrap_or(0),
        }
    }
}

impl LayoutEngine<Info> for Padded {
    fn pre_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        _parent: &RenderObject<Info>
    ) {
        let width = obj.get_value::<i32>("width");
        let height = obj.get_value::<i32>("height");
        obj.draw_rect = Rect {
            x: obj.get_value::<i32>("x").unwrap_or(0),
            y: obj.get_value::<i32>("y").unwrap_or(0),
            width: width.or_else(|| obj.get_value::<i32>("min_width"))
                .unwrap_or(0),
            height: height.or_else(|| obj.get_value::<i32>("min_height"))
                .unwrap_or(0),
        };
        obj.min_size = (
            obj.draw_rect.width,
            obj.draw_rect.height,
        );
        obj.max_size = (
            width.or_else(|| obj.get_value::<i32>("max_width")),
            height.or_else(|| obj.get_value::<i32>("max_height")),
        );
    }
    fn post_position_child(
        &mut self,
        _obj: &mut RenderObject<Info>,
        _parent: &RenderObject<Info>
    ) {
    }
    fn finalize_layout(
        &mut self,
        obj: &mut RenderObject<Info>,
        children: Vec<&mut RenderObject<Info>>
    ) {
        use std::cmp;
        let mut max = obj.min_size;
        for c in &children {
            max.0 = cmp::max(max.0, c.draw_rect.x + c.draw_rect.width);
            max.1 = cmp::max(max.1, c.draw_rect.y + c.draw_rect.height);
        }
        if let Some(v) = obj.max_size.0 {
            max.0 = cmp::min(v, max.0);
        }
        if let Some(v) = obj.max_size.1 {
            max.1 = cmp::min(v, max.1);
        }
        obj.draw_rect.width = max.0 + self.padding * 2;
        obj.draw_rect.height = max.1 + self.padding * 2;
        for c in children {
            c.draw_rect.x += self.padding;
            c.draw_rect.y += self.padding;
        }
    }
}

pub struct Rows {
    height: i32,
    adjust: bool,
}

impl Rows {
    pub fn new(obj: &RenderObject<Info>) -> Rows {
        Rows {
            height: 0,
            adjust: obj.get_value("auto_size").unwrap_or(true),
        }
    }
}

impl LayoutEngine<Info> for Rows {
    fn pre_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        parent: &RenderObject<Info>
    ) {
        obj.draw_rect = Rect {
            x: 0,
            y: self.height,
            width: parent.draw_rect.width,
            height: obj.get_value::<i32>("height").unwrap_or(0),
        };
    }
    fn post_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        _parent: &RenderObject<Info>
    ) {
        self.height += obj.draw_rect.height;
    }
    fn finalize_layout(
        &mut self,
        obj: &mut RenderObject<Info>,
        _children: Vec<&mut RenderObject<Info>>
    ) {
        if self.adjust {
            obj.draw_rect.height = self.height;
        }
    }
}

pub struct Clipped;

fn apply_clip(
    obj: &mut RenderObject<Info>,
    parent: &RenderObject<Info>
) {
    let wc = obj.get_value::<i32>("width_clip").unwrap_or(0);
    if obj.draw_rect.x < wc {
        obj.draw_rect.width += obj.draw_rect.x - wc;
        obj.draw_rect.x = 0;
    }
    if obj.draw_rect.x + obj.draw_rect.width > parent.draw_rect.width - wc {
        obj.draw_rect.width = parent.draw_rect.width - wc - obj.draw_rect.x;
    }


    let hc = obj.get_value::<i32>("height_clip").unwrap_or(0);
    if obj.draw_rect.y < hc {
        obj.draw_rect.height += obj.draw_rect.y - hc;
        obj.draw_rect.y = 0;
    }
    if obj.draw_rect.y + obj.draw_rect.height > parent.draw_rect.height - hc {
        obj.draw_rect.height = parent.draw_rect.height - hc - obj.draw_rect.y;
    }
}

impl LayoutEngine<Info> for Clipped {
    fn pre_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        parent: &RenderObject<Info>
    ) {
        let width = obj.get_value::<i32>("width");
        let height = obj.get_value::<i32>("height");
        obj.draw_rect = Rect {
            x: obj.get_value::<i32>("x").unwrap_or(0),
            y: obj.get_value::<i32>("y").unwrap_or(0),
            width: width.or_else(|| obj.get_value::<i32>("min_width"))
                .unwrap_or(0),
            height: height.or_else(|| obj.get_value::<i32>("min_height"))
                .unwrap_or(0),
        };
        obj.min_size = (
            obj.draw_rect.width,
            obj.draw_rect.height,
        );
        obj.max_size = (
            width.or_else(|| obj.get_value::<i32>("max_width")),
            height.or_else(|| obj.get_value::<i32>("max_height")),
        );
        apply_clip(obj, parent);
    }
    fn post_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        parent: &RenderObject<Info>
    ) {
        apply_clip(obj, parent);
    }
    fn finalize_layout(
        &mut self,
        _obj: &mut RenderObject<Info>,
        _children: Vec<&mut RenderObject<Info>>
    ) {

    }
}

pub struct PushBottom;
impl LayoutEngine<Info> for PushBottom {
    fn pre_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        _parent: &RenderObject<Info>
    ) {
        obj.draw_rect.width = obj.get_value("width").unwrap_or(0);
        obj.draw_rect.height = obj.get_value("height").unwrap_or(0);
    }
    fn post_position_child(
        &mut self,
        obj: &mut RenderObject<Info>,
        parent: &RenderObject<Info>
    ) {
        obj.draw_rect.y = parent.draw_rect.height - obj.draw_rect.height;
    }
    fn finalize_layout(
        &mut self,
        _obj: &mut RenderObject<Info>,
        _children: Vec<&mut RenderObject<Info>>
    ) {

    }
}