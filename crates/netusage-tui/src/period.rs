//! Los cuatro periodos consultables y su navegación.

/// Periodo seleccionable en la interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    Week,
    Month,
    LastMonth,
}

impl Period {
    /// Etiqueta legible del periodo.
    pub fn label(&self) -> &'static str {
        match self {
            Period::Today => "Hoy",
            Period::Week => "Semana",
            Period::Month => "Mes",
            Period::LastMonth => "Mes anterior",
        }
    }

    /// Los periodos en orden de presentación.
    pub fn all() -> [Period; 4] {
        [
            Period::Today,
            Period::Week,
            Period::Month,
            Period::LastMonth,
        ]
    }

    /// Siguiente periodo (cicla al primero tras el último).
    pub fn next(&self) -> Period {
        let all = Period::all();
        let idx = all.iter().position(|p| p == self).unwrap();
        all[(idx + 1) % all.len()]
    }

    /// Periodo anterior (cicla al último antes del primero).
    pub fn prev(&self) -> Period {
        let all = Period::all();
        let idx = all.iter().position(|p| p == self).unwrap();
        all[(idx + all.len() - 1) % all.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::Period;

    #[test]
    fn next_cycles_with_wraparound() {
        assert_eq!(Period::Today.next(), Period::Week);
        assert_eq!(Period::Week.next(), Period::Month);
        assert_eq!(Period::Month.next(), Period::LastMonth);
        assert_eq!(Period::LastMonth.next(), Period::Today);
    }

    #[test]
    fn prev_cycles_with_wraparound() {
        assert_eq!(Period::Today.prev(), Period::LastMonth);
        assert_eq!(Period::Week.prev(), Period::Today);
        assert_eq!(Period::LastMonth.prev(), Period::Month);
    }

    #[test]
    fn all_lists_four_in_order() {
        assert_eq!(
            Period::all(),
            [
                Period::Today,
                Period::Week,
                Period::Month,
                Period::LastMonth
            ]
        );
    }
}
