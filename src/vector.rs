use cgmath::{Array, Vector2, Vector3, Vector4, num_traits::Pow};

pub type TwoDeeVec<Type> = Vec<Vec<Type>>;
pub type ThreeDeeVec<Type> = Vec<Vec<Vec<Type>>>;

pub fn vec_two_dee<Type: std::clone::Clone>(side_len: usize, content: Type) -> TwoDeeVec<Type> {
    vec![vec![content; side_len]; side_len]
}

pub fn vec_three_dee<Type: std::clone::Clone>(side_len: usize, content: Type) -> ThreeDeeVec<Type> {
    vec![vec![vec![content; side_len]; side_len]; side_len]
}

pub trait Mask {
    fn to_index(&self, side_len: f32) -> usize;
    fn from_index(index: usize, side_len: f32) -> Self;
    fn add_dir(&self, dir: Self) -> Self;
}

impl Mask for Vector3<f32> {
    fn to_index(&self, side_len: f32) -> usize {
        Self {
            x: ((self.x) % side_len),
            y: ((self.y) % side_len) * side_len.pow(2),
            z: ((self.z) % side_len) * side_len,
        }
        .sum() as usize
    }

    fn from_index(index: usize, side_len: f32) -> Self {
        let index = index as f32;
        Self {
            x: (index % side_len.pow(2)) % side_len,
            y: index / side_len.pow(2),
            z: (index % side_len.pow(2)) / side_len,
        }
    }

    fn add_dir(&self, dir: Self) -> Self {
        Self {
            x: (self.x - dir.x).abs(),
            y: (self.y - dir.y).abs(),
            z: (self.z - dir.z).abs(),
        }
    }
}

trait Num {
    // Move Number back into boundary
    fn boundary(&self, min: Self, max: Self) -> Self;
}

impl Num for f32 {
    fn boundary(&self, min: Self, max: Self) -> Self {
        let mut corrected = self.clone();
        if self < &min {
            corrected = min
        }
        if self > &max {
            max
        } else {
            corrected
        }
    }
}

pub trait Vector {
    fn step(&self, edge: Self) -> Self;
    fn floor(&self) -> Self;
    fn sign(&self) -> Self;

    // Move Vector back into boundary
    fn boundary(&self, min: Self, max: Self) -> Self;

    fn default() -> Self;

    fn any(&self, condition: fn(f32) -> bool) -> bool;
}

impl Vector for Vector4<f32> {
    fn step(&self, edge: Self) -> Self {
        Self {
            x: (edge.x < self.x).into(),
            y: (edge.y < self.y).into(),
            z: (edge.z < self.z).into(),
            w: (edge.w < self.w).into(),
        }
    }

    fn floor(&self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
            z: self.z.floor(),
            w: self.w.floor(),
        }
    }

    fn sign(&self) -> Self {
        Self {
            x: self.x.signum(),
            y: self.y.signum(),
            z: self.z.signum(),
            w: self.w.signum(),
        }
    }

    fn boundary(&self, min: Self, max: Self) -> Self {
        Self {
            x: self.x.boundary(min.x, max.x),
            y: self.y.boundary(min.y, max.y),
            z: self.z.boundary(min.z, max.z),
            w: self.w.boundary(min.w, max.w),
        }
    }

    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    fn any(&self, condition: fn(f32) -> bool) -> bool {
        condition(self.x) || condition(self.y) || condition(self.z) || condition(self.w)
    }
}

impl Vector for Vector3<f32> {
    fn step(&self, edge: Self) -> Self {
        Self {
            x: (edge.x < self.x).into(),
            y: (edge.y < self.y).into(),
            z: (edge.z < self.z).into(),
        }
    }

    fn floor(&self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
            z: self.z.floor(),
        }
    }

    fn sign(&self) -> Self {
        Self {
            x: self.x.signum(),
            y: self.y.signum(),
            z: self.z.signum(),
        }
    }

    fn boundary(&self, min: Self, max: Self) -> Self {
        Self {
            x: self.x.boundary(min.x, max.x),
            y: self.y.boundary(min.y, max.y),
            z: self.z.boundary(min.z, max.z),
        }
    }

    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    fn any(&self, condition: fn(f32) -> bool) -> bool {
        condition(self.x) || condition(self.y) || condition(self.z)
    }
}

impl Vector for Vector2<f32> {
    fn step(&self, edge: Self) -> Self {
        Self {
            x: (edge.x < self.x).into(),
            y: (edge.y < self.y).into(),
        }
    }

    fn floor(&self) -> Self {
        Self {
            x: self.x.floor(),
            y: self.y.floor(),
        }
    }

    fn sign(&self) -> Self {
        Self {
            x: self.x.signum(),
            y: self.y.signum(),
        }
    }

    fn boundary(&self, min: Self, max: Self) -> Self {
        Self {
            x: self.x.boundary(min.x, max.x),
            y: self.y.boundary(min.y, max.y),
        }
    }

    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    fn any(&self, condition: fn(f32) -> bool) -> bool {
        condition(self.x) || condition(self.y)
    }
}