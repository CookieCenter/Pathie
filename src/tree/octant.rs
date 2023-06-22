use crate::{read_bitrange, set_bit, bitcheck};

/// Bit 1 - 16 | Child offset
/// Bit 17 - 24 | Child bitmask
/// Bit 25 | Leaf?
/// Bit 26 | Subdivide?

pub trait Octant {
    fn set(&self, leaf: bool, subdiv: bool) -> Self;
    fn has_children(&self) -> bool;
    fn is_subdiv(&self) -> bool;
    fn is_leaf(&self) -> bool;
}

impl Octant for u32 {
    /// Set leaf and subdiv either
    /// 0 for no / 1 for yes

    fn set(&self, leaf: bool, subdiv: bool) -> Self {
        let mut new = self.clone();

        // Set 25 Bit with leaf value
        new = set_bit!(new, 24, leaf);
        // Set 26 Bit with subdiv value
        new = set_bit!(new, 25, subdiv);

        new
    }

    fn has_children(&self) -> bool {
        // Extract child bitmask bitrange from self
        // Check if no value = 1
        read_bitrange!(self, 17, 24) > 0
    }

    fn is_leaf(&self) -> bool {
        bitcheck!(self, 24)
    }

    fn is_subdiv(&self) -> bool {
        bitcheck!(self, 25)
    }
}
