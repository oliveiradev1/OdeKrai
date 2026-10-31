// Hit-test geométrico desenvolvido independentemente, inspirado em
// windows/src-tauri/src/island.rs, de Coucou (Louis-CFM/coucou, MIT).
// Referência MIT: Louis-CFM/coucou, windows-v0.5.3, © Louis Raillé, 2026.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpriteRect {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) const HIT_MARGIN: f64 = 6.0;
pub(crate) const NEAR_POLL_INTERVAL: Duration = Duration::from_millis(16);
pub(crate) const FAR_POLL_INTERVAL: Duration = Duration::from_millis(60);

pub(crate) fn contains(rect: SpriteRect, x: f64, y: f64) -> bool {
    x >= rect.x - HIT_MARGIN
        && x <= rect.x + rect.width + HIT_MARGIN
        && y >= rect.y - HIT_MARGIN
        && y <= rect.y + rect.height + HIT_MARGIN
}

/// No retângulo ou coordenada disponível, a janela continua aceitando cliques.
pub(crate) fn should_ignore_cursor_events(
    enabled: bool,
    rect: Option<SpriteRect>,
    cursor: Option<(f64, f64)>,
) -> bool {
    match (enabled, rect, cursor) {
        (true, Some(rect), Some((x, y))) => !contains(rect, x, y),
        _ => false,
    }
}

/// `None` instrui o monitor a aguardar Notify enquanto a janela está inativa.
pub(crate) fn poll_interval(
    visible: bool,
    enabled: bool,
    cursor_in_hit_area: bool,
) -> Option<Duration> {
    if !visible || !enabled {
        None
    } else if cursor_in_hit_area {
        Some(NEAR_POLL_INTERVAL)
    } else {
        Some(FAR_POLL_INTERVAL)
    }
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

    #[test]
    fn intervalo_do_monitor_reduz_consultas_longe_e_aguarda_pet_inativo() {
        assert_eq!(
            poll_interval(true, true, false),
            Some(Duration::from_millis(60))
        );
        assert_eq!(
            poll_interval(true, true, true),
            Some(Duration::from_millis(16))
        );
        assert_eq!(poll_interval(false, true, true), None);
        assert_eq!(poll_interval(true, false, true), None);
    }

    #[test]
    fn hit_test_sem_retangulo_inicial_aceita_cliques() {
        assert!(!should_ignore_cursor_events(true, None, Some((10.0, 10.0))));
        assert!(!should_ignore_cursor_events(true, None, None));
        assert!(!should_ignore_cursor_events(
            false,
            None,
            Some((10.0, 10.0))
        ));
    }
}
