use cgmath::{Vector3, Vector4};

use crate::service::{Vector, Mask};

pub const MAX_RECURSION: usize = 17;
pub const ROOT_SPAN: f32 = (1 << MAX_RECURSION) as f32;

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct TreeNode {
    // 0 = empty | 1 = subdivide | 2 = full | 3 = light
    pub node_type: u32,
    pub parent: u32,
    
    pub children: [u32; 8],
    pub base_color: Vector4<f32>,
}

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Ray {
    pub origin: Vector4<f32>,
    pub dir: Vector4<f32>,
}

#[repr(C)]
#[derive(Clone, Debug, Copy)]
pub struct Traverse {
    pub parent: u32,
    pub index: u32,

    pub span: f32,
    
    pub depth: i32,
    pub mask_in_parent: [Vector4<i32>; MAX_RECURSION], // Position in parent at depth

    pub ray: Ray,
    pub dist: f32,

    pub local_origin: Vector4<f32>, // Origin in CurNode
    pub origin_on_edge: Vector4<f32>, // Origin on first edge of CurNode
}

pub struct Octree {
    // RootIndex = 0
    pub data: Vec<TreeNode>, // Octree as List
    pub light_data: Vec<Traverse>,
}

impl TreeNode {
    pub fn new(node_type: u32, parent: usize, ) -> TreeNode {
        TreeNode { node_type, parent: parent as u32, .. Default::default() }
    }

    pub fn set_full(&mut self, base_color: Vector4<f32>, node_type: u32, ) {
        self.node_type = node_type;
        self.base_color = base_color;
    }

    pub fn get_center(node_pos: Vector4<f32>, span: f32, ) -> Vector4<f32> {
        node_pos + Vector4::from([span * 0.5; 4])
    }

    pub fn get_top_right(node_pos: Vector4<f32>, span: f32, ) -> Vector4<f32> {
        node_pos + Vector4::from([span; 4])
    }

    pub fn get_child_mask(cur_span: f32, local_origin: Vector4<f32>, ) -> Vector4<i32> {
        Vector4::from([cur_span; 4]).step(local_origin)
            .cast::<i32>()
            .unwrap()
    }
}

impl Octree {
    pub fn node_at_pos(&mut self, pos: Vector4<f32>, ) -> Traverse {
        let mut traverse = Traverse {
            span: ROOT_SPAN,

            local_origin: pos % ROOT_SPAN,
            origin_on_edge: pos - (pos % ROOT_SPAN),

            .. Default::default()
        };

        for _  in 1 .. MAX_RECURSION {
            if traverse.node_type(&self.data) == 1 {
                traverse.move_into_child(&self.data);
            } else {
                break;
            }
        }

        traverse
    }

    pub fn insert_node(&mut self, insert_pos: Vector4<f32>, base_color: Vector4<f32>, node_type: u32, ) -> Traverse {
        let mut traverse = Traverse {
            span: ROOT_SPAN,

            local_origin: insert_pos % ROOT_SPAN,
            origin_on_edge: insert_pos - (insert_pos % ROOT_SPAN),

            .. Default::default()
        };

        for _  in 1 .. MAX_RECURSION {
            traverse.try_child_creation(&mut self.data);
            traverse.move_into_child(&self.data);
            traverse.update_color(&mut self.data, base_color)
        }

        self.data[traverse.index()].set_full(base_color.clone(), node_type, );

        traverse
    }

    pub fn insert_light(&mut self, insert_pos: Vector4<f32>, light_color: Vector4<f32>, ) {
        let traverse = self.insert_node(insert_pos, light_color, 3, );
        self.light_data.push(traverse.clone());
    }

    pub fn test_scene(&mut self) {
        // Ground
        for x in 0 .. 100 {
            for z in 0 .. 100 {
                let base_color = Vector4::new(1.0, 1.0, 1.0, 0.0, );
                self.insert_node(Vector4::new(100.0 + x as f32, 100.0, 100.0 + z as f32, 0.0, ), base_color, 2);
            }
        }

        // GreenWall
        for z in 0 .. 100 {
            for y in 0 .. 100 {
                let base_color = Vector4::new(0.0, 1.0, 0.0, 0.0, );
                self.insert_node(Vector4::new(100.0, 100.0 + y as f32, 100.0 + z as f32, 0.0, ), base_color, 2);
            }
        }

        // RedWall
        for z in 0 .. 100 {
            for y in 0 .. 100 {
                let base_color = Vector4::new(1.0, 0.0, 0.0, 0.0, );
                self.insert_node(Vector4::new(200.0, 100.0 + y as f32, 100.0 + z as f32, 0.0, ), base_color, 2);
            }
        }

        // BackWall
        for x in 0 .. 100 {
            for y in 0 .. 100 {
                let base_color = Vector4::new(1.0, 1.0, 1.0, 0.0, );
                self.insert_node(Vector4::new(100.0 + x as f32, 100.0 + y as f32, 200.0, 0.0, ), base_color, 2);
            }
        }

        // Ceilling
        for x in 0 .. 100 {
            for z in 0 .. 100 {
                let base_color = Vector4::new(1.0, 1.0, 1.0, 0.0, );
                self.insert_node(Vector4::new(100.0 + x as f32, 200.0, 100.0 + z as f32, 0.0, ), base_color, 2);
            }
        }

        // Box
        for x in 0 .. 20 {
            for z in 0 .. 20 {
                for y in 0 .. 20 {
                    let base_color = Vector4::new(0.0, 0.0, 1.0, 0.0, );
                    self.insert_node(Vector4::new(140.0 + x as f32, 100.0 + y as f32, 140.0 + z as f32, 0.0, ), base_color, 2);
                }
            }
        }

        // Light
        let light_color = Vector4::new(1.0, 1.0, 0.0, 0.0, );
        self.insert_light(Vector4::new(150.0, 180.0, 150.0, 0.0, ), light_color, );
    }
}

impl Traverse {
    pub fn index(&self) -> usize { self.index as usize }
    pub fn parent(&self) -> usize { self.parent as usize }
    pub fn node_type(&self, data: &Vec<TreeNode>, ) -> u32 { data[self.index()].node_type }

    pub fn update_color(&mut self, data: &mut Vec<TreeNode>, color: Vector4<f32>, ) {
        data[self.index()].base_color += color / self.span;
    }

    pub fn try_child_creation(&mut self, data: &mut Vec<TreeNode>, ) {
        if data[self.index()].node_type == 0 {
            data[self.index()].node_type = 1;

            for index in 0 .. 8 {
                data[self.index()].children[index] = data.len() as u32;
                data.push(TreeNode::new(0, self.index(), ));
            }
        }
    }

    pub fn move_into_child(&mut self, data: &Vec<TreeNode>, ) {
        self.span *= 0.5;
        self.depth += 1;

        let child_mask =
            TreeNode::get_child_mask(self.span, self.local_origin, );

        self.origin_on_edge += child_mask.cast::<f32>().unwrap() * self.span;
        self.local_origin -= child_mask.cast::<f32>().unwrap() * self.span;

        self.mask_in_parent[self.depth as usize] = child_mask.clone();
        let space_index = child_mask.to_index(2);

        self.parent = self.index;
        self.index = data[self.parent()].children[space_index] as u32;
    }
}

impl Default for TreeNode {
    fn default() -> Self {
        Self { 
            node_type: 0,
            parent: 0,
            children: [0; 8],
            base_color: Vector4::default()
        }
    }
}

impl Default for Ray {
    fn default() -> Self {
        Self {
            origin: Vector4::default(),
            dir: Vector4::default()
        }
    }
}

impl Default for Traverse {
    fn default() -> Self {
        Self { 
            parent: 0,
            index: 0,

            span: ROOT_SPAN,

            depth: 0,
            mask_in_parent: [Vector4::from([0; 4]); MAX_RECURSION],

            ray: Ray::default(),
            dist: 0.0,

            local_origin: Vector4::default(),
            origin_on_edge: Vector4::default(),
        }
    }
}

impl Default for Octree {
    fn default() -> Self {
        log::info!("Creating Octree with RootSpan [ {} ] -> VoxSpan is 1.0 ...", ROOT_SPAN);
        Self { data: vec![TreeNode::default()], light_data: vec![] }
    }
}