// Hit-test geométrico desenvolvido independentemente, inspirado em
// windows/src-tauri/src/island.rs, de Coucou (Louis-CFM/coucou, MIT).
// Referência MIT: Louis-CFM/coucou, windows-v0.5.3, © Louis Raillé, 2026.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpriteRect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) const HIT_MARGIN: f64 = 6.0;

pub(crate) fn contains(rect: SpriteRect, x: f64, y: f64) -> bool {
    x >= rect.x - HIT_MARGIN
        && x <= rect.x + rect.width + HIT_MARGIN
        && y >= rect.y - HIT_MARGIN
        && y <= rect.y + rect.height + HIT_MARGIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_aceita_sprite_e_margem_e_rejeita_area_transparente() {
        let sprite = SpriteRect {
            x: 20.0,
            y: 30.0,
            width: 66.0,
            height: 66.0,
        };
        assert!(contains(sprite, 40.0, 50.0));
        assert!(contains(sprite, 14.0, 30.0));
        assert!(contains(sprite, 92.0, 102.0));
        assert!(!contains(sprite, 13.99, 30.0));
        assert!(!contains(sprite, 93.0, 50.0));
        assert!(!contains(sprite, 40.0, 103.0));
    }
}
