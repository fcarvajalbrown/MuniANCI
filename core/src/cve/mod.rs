//! Offline CVE enrichment.
//!
//! Convierte el inventario de software que ya releva el escáner en CVE concretas,
//! sin salir del equipo, contra un snapshot de NVD empaquetado con el producto.
//!
//! ## Avisos de atribución (obligatorios)
//!
//! Los términos de uso exigen mostrarlos donde se presenten estos datos. Ver
//! [`NVD_NOTICE`] y [`CVE_NOTICE`].
//!
//! ## Por qué el mapeo nombre -> CPE es curado y no automático
//!
//! El inventario entrega lo que reporta el sistema operativo ("Microsoft SQL
//! Server 2014 (64-bit)"); NVD necesita un CPE. Esa traducción es la principal
//! fuente de falsos positivos de todo escáner de vulnerabilidades, sobre todo por
//! confusión entre ecosistemas: paquetes de nombre parecido en plataformas
//! distintas. En un informe que un municipio podría presentar ante la ANCI, un
//! falso positivo cuesta más que un hueco declarado.
//!
//! Por eso la tabla es explícita y auditable: si un producto no está mapeado, el
//! informe dice que no se evaluó, en vez de arriesgar una afirmación falsa. La
//! cobertura se declara en [`Coverage`] y debe mostrarse en el informe.

pub mod cpe;
pub mod matcher;

use serde::{Deserialize, Serialize};

/// Aviso exigido por los términos de uso de la API de NVD.
pub const NVD_NOTICE: &str =
    "This product uses the NVD API but is not endorsed or certified by the NVD.";

/// Aviso exigido por los términos de uso del CVE Program (MITRE).
pub const CVE_NOTICE: &str =
    "CVE(R) is a registered trademark of The MITRE Corporation. \
     CVE records are used under the CVE Program Terms of Use.";

/// How much of the inventory could actually be evaluated.
///
/// Se informa siempre. "42 de 180 paquetes evaluados" es honesto; mostrar
/// hallazgos para los 180 cuando 138 no se pudieron mapear, no lo es.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    /// Paquetes del inventario considerados.
    pub total: usize,
    /// Paquetes que se pudieron mapear a un CPE y por tanto evaluar.
    pub evaluated: usize,
}

impl Coverage {
    /// Packages that could not be mapped to a CPE.
    pub fn unmapped(&self) -> usize {
        self.total.saturating_sub(self.evaluated)
    }

    /// Percentage of the inventory actually evaluated, 0 when there is nothing.
    pub fn percent(&self) -> f32 {
        if self.total == 0 {
            return 0.0;
        }
        (self.evaluated as f32 / self.total as f32) * 100.0
    }
}

impl std::fmt::Display for Coverage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} de {} paquete(s) evaluado(s) ({:.0}%); {} sin mapeo a CPE",
            self.evaluated,
            self.total,
            self.percent(),
            self.unmapped()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_reports_what_was_left_out() {
        let c = Coverage { total: 180, evaluated: 42 };
        assert_eq!(c.unmapped(), 138);
        assert!(c.to_string().contains("42 de 180"));
        assert!(c.to_string().contains("138 sin mapeo"));
    }

    #[test]
    fn empty_inventory_does_not_divide_by_zero() {
        let c = Coverage::default();
        assert_eq!(c.percent(), 0.0);
        assert_eq!(c.unmapped(), 0);
    }

    #[test]
    fn full_coverage_is_one_hundred_percent() {
        let c = Coverage { total: 10, evaluated: 10 };
        assert_eq!(c.percent(), 100.0);
        assert_eq!(c.unmapped(), 0);
    }
}
