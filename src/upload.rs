//! Derived GPU upload decisions. Identity is owned by the explicit immutable scene.
use std::rc::{Rc, Weak};

#[derive(Default)]
pub struct Upload {
    scene: Weak<()>,
    input: Vec<u32>,
    selected: Vec<u32>,
}
impl Upload {
    pub fn plan(&mut self, scene: &Rc<()>, input: &[u32]) -> Option<Vec<u32>> {
        let same = self.scene.ptr_eq(&Rc::downgrade(scene));
        if same && self.input == input {
            return None;
        }
        let mut selected = input.to_vec();
        selected.sort_unstable();
        selected.dedup();
        if same && self.selected == selected {
            self.input = input.to_vec();
            return None;
        }
        Some(selected)
    }
    /// Commit only after validation and queuing the GPU buffer write.
    pub fn commit(&mut self, scene: &Rc<()>, input: &[u32], selected: Vec<u32>) {
        self.scene = Rc::downgrade(scene);
        self.input = input.to_vec();
        self.selected = selected;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_identity_and_selection_sets_control_uploads() {
        let scene = Rc::new(());
        let mut upload = Upload::default();
        let first = upload.plan(&scene, &[3, 1, 3]);
        assert_eq!(first, Some(vec![1, 3]));
        upload.commit(&scene, &[3, 1, 3], vec![1, 3]);
        assert_eq!(upload.selected, [1, 3]);
        assert!(upload.plan(&scene, &[3, 1, 3]).is_none());
        assert!(
            upload.plan(&scene, &[1, 3]).is_none(),
            "selection order caused an unnecessary scene upload"
        );
        assert!(upload.plan(&scene, &[1, 3]).is_none());
        assert_eq!(upload.plan(&scene, &[1]), Some(vec![1]));
        assert_eq!(upload.plan(&Rc::new(()), &[1, 3]), Some(vec![1, 3]));
    }
    #[test]
    fn failed_plans_do_not_commit_and_dead_scene_identity_cannot_be_reused() {
        let scene = Rc::new(());
        let mut upload = Upload::default();
        assert!(upload.plan(&scene, &[99]).is_some());
        assert!(upload.plan(&scene, &[99]).is_some());
        upload.commit(&scene, &[], vec![]);
        drop(scene);
        assert_eq!(upload.scene.strong_count(), 0);
        assert!(upload.plan(&Rc::new(()), &[]).is_some());
    }
}
