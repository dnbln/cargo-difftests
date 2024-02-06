use std::sync::{Arc, Weak};

use prodash::tree::{Item, Root as Tree};

struct Shell {
    pub tree: Arc<Tree>,
}

pub struct CargoDifftestsContext {
    shell: Shell,
}

impl CargoDifftestsContext {
    pub fn new() -> (Self, Weak<Tree>) {
        let ctxt = Self {
            shell: Shell { tree: Tree::new() },
        };
        let tree = Arc::downgrade(&ctxt.shell.tree);
        (ctxt, tree)
    }

    pub fn new_child(&self, label: &str) -> Item {
        self.shell.tree.add_child(label)
    }
}
