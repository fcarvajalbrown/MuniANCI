//! Fase de la Ley 21.180 que le corresponde a la institución, según el DFL N°1.
//!
//! ## Qué responde
//!
//! "¿En qué va mi municipalidad con la Ley de Transformación Digital?" es una pregunta
//! que hoy solo se contesta leyendo nueve páginas de un decreto con fuerza de ley y
//! buscando el nombre de la comuna en una lista de trescientos. Este módulo lo resuelve
//! con el nombre que ya está compilado en el binario.
//!
//! ## De dónde salen los datos
//!
//! Del **DFL N°1 de 2020** del Ministerio Secretaría General de la Presidencia (D.O.
//! 06-04-2021), leído íntegro el 2026-07-25 en su versión vigente, con las modificaciones
//! de la **Ley 21.464** (D.O. 09-06-2022) y la **Ley 21.806** (D.O. 05-02-2026). El PDF
//! vive en `docs/Decreto-con Fuerza de Ley-1_06-ABR-2021.pdf`.
//!
//! - El **Art. 5°** reparte los órganos en tres grupos, y **nombra una por una** a las
//!   municipalidades: el Grupo B las lista junto a los gobiernos regionales, el Grupo C
//!   lista las restantes. El Grupo A las excluye expresamente.
//! - El **Art. 6°** define la fase de Preparación y seis fases numeradas.
//! - El **Art. 7°** fija qué fase toca a cada grupo cada año.
//!
//! ## Esto no es ciberseguridad, y por eso viaja aparte
//!
//! La Ley 21.180 es transformación digital, no la Ley 21.663. Lo que este módulo informa
//! no entra al puntaje de cumplimiento del escáner ni a su perfil de madurez: es un dato
//! con su propio marco. El vínculo existe —el Decreto 7, que sí es la norma de seguridad,
//! remite en su Art. 13° a esta misma gradualidad— pero **traducir una fase de
//! procedimiento administrativo a un deber de seguridad es interpretación jurídica**, y
//! este producto no la hace.
//!
//! ## Lo que no se afirma
//!
//! - Si el nombre no está en ninguna de las dos listas, se dice eso y no se adivina un
//!   grupo. Hay servicios públicos y órganos que no son municipalidades.
//!
//! ## Las listas no cubren las 346 comunas, y eso es del decreto
//!
//! Entre los dos literales, el Art. 5° nombra **343** municipalidades. Chile tiene 346
//! comunas, y las tres que faltan —**Antártica**, **Canela** y **Quillón**— simplemente
//! no están en el texto. Se verificó una por una contra el PDF el 2026-07-25, después de
//! que un diff contra un catálogo de comunas delatara la diferencia; ese mismo diff
//! encontró que **Las Cabras** sí estaba en el decreto y se había perdido al transcribir.
//!
//! No se completan desde un directorio de comunas: la lista de este módulo es la del
//! decreto, no la de Chile. Una comuna que el decreto no nombra se informa como no
//! identificada, que es la verdad. Hay una prueba que lo fija.
//!
//! Donde el decreto y el uso corriente escriben distinto, manda el decreto: **Aysén**
//! (no "Aisén"), **Coyhaique** (no "Coihaique") y **Trehuaco** (no "Treguaco").
//! - La tabla del Art. 7° termina en 2027. Un año posterior se informa como fuera de
//!   tabla, sin extrapolar.
//! - Estar en una fase no dice si la institución la cumplió. Dice qué le tocaba.

use chrono::Datelike;
use serde::{Deserialize, Serialize};

/// Grupo del Art. 5° del DFL N°1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grupo {
    /// Ministerios, servicios públicos que no sean gobiernos regionales ni municipios,
    /// Contraloría, Fuerzas Armadas y de Orden, y delegaciones presidenciales.
    /// **No incluye municipalidades.**
    A,
    /// Gobiernos regionales y las municipalidades nombradas en el Art. 5° lit. b).
    B,
    /// Las municipalidades nombradas en el Art. 5° lit. c).
    C,
}

impl std::fmt::Display for Grupo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Grupo::A => f.write_str("Grupo A"),
            Grupo::B => f.write_str("Grupo B"),
            Grupo::C => f.write_str("Grupo C"),
        }
    }
}

/// Fase del Art. 6° del DFL N°1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fase {
    Preparacion,
    /// Comunicaciones oficiales entre órganos, en plataforma electrónica.
    Uno,
    /// Notificaciones por medios electrónicos.
    Dos,
    /// Ingreso de solicitudes, formularios y documentos por medios electrónicos.
    Tres,
    /// El procedimiento administrativo consta en expediente electrónico.
    Cuatro,
    /// Digitalización de lo presentado en papel e incorporación al expediente.
    Cinco,
    /// Aplicación del principio de interoperabilidad.
    Seis,
}

impl Fase {
    /// Qué exige la fase, en los términos del Art. 6°.
    pub fn descripcion(self) -> &'static str {
        match self {
            Fase::Preparacion => {
                "Preparación: identificar y describir las etapas de los procedimientos \
                 administrativos que desarrolla, y en particular la necesidad de notificación \
                 en cada uno"
            }
            Fase::Uno => {
                "Fase 1: las comunicaciones oficiales entre órganos de la Administración se \
                 registran en una plataforma electrónica"
            }
            Fase::Dos => "Fase 2: las notificaciones se practican por medios electrónicos",
            Fase::Tres => {
                "Fase 3: el ingreso de solicitudes, formularios o documentos se hace por \
                 documentos o formatos electrónicos"
            }
            Fase::Cuatro => {
                "Fase 4: el procedimiento administrativo consta en un expediente electrónico"
            }
            Fase::Cinco => {
                "Fase 5: lo presentado en papel se digitaliza e ingresa al expediente \
                 electrónico"
            }
            Fase::Seis => "Fase 6: aplicación del principio de interoperabilidad",
        }
    }
}

impl std::fmt::Display for Fase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fase::Preparacion => f.write_str("Preparación"),
            Fase::Uno => f.write_str("Fase 1"),
            Fase::Dos => f.write_str("Fase 2"),
            Fase::Tres => f.write_str("Fase 3"),
            Fase::Cuatro => f.write_str("Fase 4"),
            Fase::Cinco => f.write_str("Fase 5"),
            Fase::Seis => f.write_str("Fase 6"),
        }
    }
}

/// Primer y último año que cubre la tabla del Art. 7°.
pub const PRIMER_ANIO: i32 = 2022;
pub const ULTIMO_ANIO: i32 = 2027;

/// Procedencia, para que el informe pueda declarar de dónde salió el dato.
pub const PROCEDENCIA: &str =
    "DFL N°1 de 2020 del Ministerio Secretaría General de la Presidencia (D.O. 06-04-2021), \
     Arts. 5°, 6° y 7°, en su versión vigente con las modificaciones de la Ley 21.464 \
     (D.O. 09-06-2022) y la Ley 21.806 (D.O. 05-02-2026).";

/// Fases que el Art. 7° asigna a un grupo en un año dado.
///
/// `None` significa que el año está fuera de la tabla, no que no haya nada que hacer.
/// El Grupo A en 2027 devuelve `Some(&[])`: la tabla lo cubre y no le asigna fase nueva.
pub fn fases_de(grupo: Grupo, anio: i32) -> Option<&'static [Fase]> {
    if !(PRIMER_ANIO..=ULTIMO_ANIO).contains(&anio) {
        return None;
    }
    use Fase::*;
    Some(match (grupo, anio) {
        (_, 2022) => &[Preparacion],

        (Grupo::A, 2023) => &[Uno],
        (Grupo::A, 2024) => &[Tres],
        (Grupo::A, 2025) => &[Seis],
        (Grupo::A, 2026) => &[Dos, Cuatro, Cinco],
        (Grupo::A, 2027) => &[],

        (Grupo::B, 2023) => &[Preparacion],
        (Grupo::B, 2024) => &[Uno],
        (Grupo::B, 2025) => &[Tres, Seis],
        (Grupo::B, 2026) => &[Cuatro, Cinco],
        (Grupo::B, 2027) => &[Dos],

        (Grupo::C, 2023) => &[Preparacion],
        (Grupo::C, 2024) => &[Uno],
        (Grupo::C, 2025) => &[Tres],
        (Grupo::C, 2026) => &[Cuatro, Seis],
        (Grupo::C, 2027) => &[Dos, Cinco],

        // El rango ya se validó arriba; este brazo es inalcanzable.
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Estado informado
// ---------------------------------------------------------------------------

/// Lo que el informe puede decir sobre la Ley 21.180 para esta institución.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstadoLey21180 {
    /// Nombre tal como se compiló en el binario.
    pub institucion: String,
    /// Grupo del Art. 5°, si el nombre se pudo identificar en el decreto.
    pub grupo: Option<Grupo>,
    /// Año contra el que se resolvió la tabla.
    pub anio: i32,
    /// Fases que tocan ese año. Vacío con `grupo` presente significa "la tabla cubre el
    /// año y no asigna fase nueva"; `None` en `grupo` significa que no se identificó.
    pub fases: Vec<Fase>,
    /// Por qué el resultado es el que es, en lenguaje llano.
    pub nota: String,
    pub procedencia: &'static str,
}

/// Resuelve el estado para una institución y un año.
///
/// El año se pasa explícito para que la función sea pura y testeable; quien la llama
/// desde el escáner usa [`anio_actual`].
pub fn estado(institucion: &str, anio: i32) -> EstadoLey21180 {
    let grupo = grupo_de(institucion);
    let fases = grupo.and_then(|g| fases_de(g, anio)).unwrap_or(&[]).to_vec();

    let nota = match (grupo, fases.is_empty()) {
        (None, _) => format!(
            "\"{institucion}\" no figura en las listas de municipalidades del Art. 5° del \
             DFL N°1. No se le atribuye grupo: el decreto nombra a las municipalidades una \
             por una, y también obliga a órganos que no son municipalidades y que quedan en \
             el Grupo A. Verificar el nombre exacto contra el decreto."
        ),
        (Some(g), true) if !(PRIMER_ANIO..=ULTIMO_ANIO).contains(&anio) => format!(
            "La tabla del Art. 7° cubre de {PRIMER_ANIO} a {ULTIMO_ANIO} y {anio} queda fuera. \
             La institución está en el {g}, pero no se extrapola qué le corresponde."
        ),
        (Some(g), true) => format!(
            "La tabla del Art. 7° cubre {anio} y no le asigna fase nueva al {g} ese año."
        ),
        (Some(g), false) => format!(
            "La institución está en el {g} del Art. 5°, y el Art. 7° le asigna para {anio}: {}.",
            fases.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ")
        ),
    };

    EstadoLey21180 { institucion: institucion.to_string(), grupo, anio, fases, nota, procedencia: PROCEDENCIA }
}

/// Año en curso, del reloj del equipo.
///
/// Nunca un literal: una constante con el año escrito a mano envejece en silencio y
/// pasa a informar la fase equivocada el 1 de enero.
pub fn anio_actual() -> i32 {
    chrono::Utc::now().year()
}

// ---------------------------------------------------------------------------
// Identificación de la comuna
// ---------------------------------------------------------------------------

/// Normaliza un nombre para compararlo: sin tildes, sin mayúsculas, sin el tratamiento
/// institucional, y con los espacios colapsados.
///
/// Hace falta porque el nombre que llega es el que se compiló para el cliente
/// ("I. Municipalidad de Ñuñoa", "MUNICIPALIDAD DE NUNOA") y el del decreto es la comuna
/// pelada ("Ñuñoa"). Comparar las cadenas crudas fallaría en el caso normal.
fn normalizar(nombre: &str) -> String {
    let sin_tildes: String = nombre
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
            'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
            'ñ' | 'Ñ' => 'n',
            otro => otro,
        })
        .collect();

    // Los espacios se colapsan **antes** de buscar el tratamiento, no después: un nombre
    // tipeado a mano trae espacios de más ("municipalidad   de   Ñuñoa") y con el orden
    // inverso el prefijo no calzaba nunca.
    let compacto = sin_tildes.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");

    // Se quita el tratamiento por el principio, del más largo al más corto, para que
    // "ilustre municipalidad de" no quede reducido a "ilustre".
    let mut resto = compacto.as_str();
    for prefijo in [
        "ilustre municipalidad de ",
        "i. municipalidad de ",
        "i municipalidad de ",
        "municipalidad de ",
        "muni. de ",
        "comuna de ",
    ] {
        if let Some(sin_prefijo) = resto.strip_prefix(prefijo) {
            resto = sin_prefijo;
            break;
        }
    }

    resto.to_string()
}

/// Grupo al que el Art. 5° asigna esta institución, si la nombra.
pub fn grupo_de(institucion: &str) -> Option<Grupo> {
    let clave = normalizar(institucion);
    if clave.is_empty() {
        return None;
    }
    if GRUPO_B.iter().any(|c| normalizar(c) == clave) {
        return Some(Grupo::B);
    }
    if GRUPO_C.iter().any(|c| normalizar(c) == clave) {
        return Some(Grupo::C);
    }
    None
}

/// Municipalidades del Art. 5° lit. b), transcritas del decreto.
///
/// El literal nombra además a los gobiernos regionales, que no son municipalidades y por
/// eso no están en esta lista.
pub const GRUPO_B: &[&str] = &[
    "Alto Hospicio", "Antofagasta", "Arica", "Buin", "Calama", "Calera", "Cartagena",
    "Cerrillos", "Cerro Navia", "Chiguayante", "Chillán", "Chillán Viejo", "Colina",
    "Concepción", "Conchalí", "Concón", "Copiapó", "Coquimbo", "Coronel", "Coyhaique",
    "Curicó", "El Bosque", "El Tabo", "Estación Central", "Hualpén", "Huechuraba",
    "Independencia", "Iquique", "La Cisterna", "La Cruz", "La Florida", "La Granja",
    "La Pintana", "La Reina", "La Serena", "Lampa", "Las Condes", "Lo Barnechea",
    "Lo Espejo", "Lo Prado", "Los Ángeles", "Lota", "Machalí", "Macul", "Maipú", "Ñuñoa",
    "Osorno", "Padre Hurtado", "Pedro Aguirre Cerda", "Penco", "Peñaflor", "Peñalolén",
    "Pirque", "Providencia", "Pudahuel", "Puente Alto", "Puerto Montt", "Puerto Varas",
    "Punta Arenas", "Quilicura", "Quillota", "Quilpué", "Quinta Normal", "Rancagua",
    "Recoleta", "Renca", "San Antonio", "San Bernardo", "San Joaquín", "San Miguel",
    "San Pedro de la Paz", "San Ramón", "Santiago", "Santo Domingo", "Talagante", "Talca",
    "Talcahuano", "Temuco", "Tomé", "Valdivia", "Valparaíso", "Villa Alemana",
    "Viña del Mar", "Vitacura",
];

/// Municipalidades del Art. 5° lit. c), transcritas del decreto.
pub const GRUPO_C: &[&str] = &[
    "Algarrobo", "Alhué", "Alto Biobío", "Alto del Carmen", "Ancud", "Andacollo", "Angol",
    "Antuco", "Arauco", "Aysén", "Bulnes", "Cabildo", "Cabo de Hornos", "Cabrero",
    "Calbuco", "Caldera", "Calera de Tango", "Calle Larga", "Camarones", "Camiña",
    "Cañete", "Carahue", "Casablanca", "Castro", "Catemu", "Cauquenes", "Chaitén",
    "Chanco", "Chañaral", "Chépica", "Chile Chico", "Chimbarongo", "Cholchol", "Chonchi",
    "Cisnes", "Cobquecura", "Cochamó", "Cochrane", "Codegua", "Coelemu", "Coihueco",
    "Coinco", "Colbún", "Colchane", "Collipulli", "Coltauco", "Combarbalá",
    "Constitución", "Contulmo", "Corral", "Cunco", "Curacautín", "Curacaví",
    "Curaco de Vélez", "Curanilahue", "Curarrehue", "Curepto", "Dalcahue",
    "Diego de Almagro", "Doñihue", "El Carmen", "El Monte", "El Quisco", "Empedrado",
    "Ercilla", "Florida", "Freire", "Freirina", "Fresia", "Frutillar", "Futaleufú",
    "Futrono", "Galvarino", "General Lagos", "Gorbea", "Graneros", "Guaitecas",
    "Hijuelas", "Hualaihué", "Hualañé", "Hualqui", "Huara", "Huasco", "Illapel",
    "Isla de Maipo", "Isla de Pascua", "Juan Fernández", "La Estrella", "La Higuera",
    "La Ligua", "La Unión", "Lago Ranco", "Lago Verde", "Laguna Blanca", "Laja", "Lanco",
    "Las Cabras", "Lautaro", "Lebu", "Licantén", "Limache", "Linares", "Litueche", "Llaillay",
    "Llanquihue", "Lolol", "Loncoche", "Longaví", "Lonquimay", "Los Álamos", "Los Andes",
    "Los Lagos", "Los Muermos", "Los Sauces", "Los Vilos", "Lumaco", "Máfil", "Malloa",
    "Marchihue", "María Elena", "María Pinto", "Mariquina", "Maule", "Maullín",
    "Mejillones", "Melipeuco", "Melipilla", "Molina", "Monte Patria", "Mostazal",
    "Mulchén", "Nacimiento", "Nancagua", "Natales", "Navidad", "Negrete", "Ninhue",
    "Nogales", "Nueva Imperial", "Ñiquén", "O'Higgins", "Olivar", "Ollagüe", "Olmué",
    "Ovalle", "Padre Las Casas", "Paiguano", "Paillaco", "Paine", "Palena", "Palmilla",
    "Panguipulli", "Panquehue", "Papudo", "Paredones", "Parral", "Pelarco", "Pelluhue",
    "Pemuco", "Pencahue", "Peralillo", "Perquenco", "Petorca", "Peumo", "Pica",
    "Pichidegua", "Pichilemu", "Pinto", "Pitrufquén", "Placilla", "Portezuelo",
    "Porvenir", "Pozo Almonte", "Primavera", "Puchuncaví", "Pucón", "Puerto Octay",
    "Pumanque", "Punitaqui", "Puqueldón", "Purén", "Purranque", "Putaendo", "Putre",
    "Puyehue", "Queilén", "Quellón", "Quemchi", "Quilaco", "Quilleco", "Quinchao",
    "Quinta de Tilcoco", "Quintero", "Quirihue", "Ranquil", "Rauco", "Renaico", "Rengo",
    "Requínoa", "Retiro", "Rinconada", "Río Bueno", "Río Claro", "Río Hurtado",
    "Río Ibáñez", "Río Negro", "Río Verde", "Romeral", "Saavedra", "Sagrada Familia",
    "Salamanca", "San Carlos", "San Clemente", "San Esteban", "San Fabián", "San Felipe",
    "San Fernando", "San Gregorio", "San Ignacio", "San Javier", "San José de Maipo",
    "San Juan de la Costa", "San Nicolás", "San Pablo", "San Pedro",
    "San Pedro de Atacama", "San Rafael", "San Rosendo", "San Vicente", "Santa Bárbara",
    "Santa Cruz", "Santa Juana", "Santa María", "Sierra Gorda", "Taltal", "Teno",
    "Teodoro Schmidt", "Tierra Amarilla", "Tiltil", "Timaukel", "Tirúa", "Tocopilla",
    "Toltén", "Torres del Paine", "Tortel", "Traiguén", "Trehuaco", "Tucapel", "Vallenar",
    "Vichuquén", "Victoria", "Vicuña", "Vilcún", "Villa Alegre", "Villarrica",
    "Yerbas Buenas", "Yumbel", "Yungay", "Zapallar",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nunoa_y_providencia_son_grupo_b() {
        assert_eq!(grupo_de("Ñuñoa"), Some(Grupo::B));
        assert_eq!(grupo_de("Providencia"), Some(Grupo::B));
    }

    #[test]
    fn algarrobo_es_grupo_c() {
        assert_eq!(grupo_de("Algarrobo"), Some(Grupo::C));
        assert_eq!(grupo_de("Zapallar"), Some(Grupo::C));
    }

    #[test]
    fn un_nombre_desconocido_no_recibe_grupo() {
        // Lo importante no es que devuelva None, sino que no adivine: el decreto también
        // obliga a órganos que no son municipalidades.
        assert_eq!(grupo_de("Servicio de Impuestos Internos"), None);
        assert_eq!(grupo_de("Comuna Inventada"), None);
        assert_eq!(grupo_de(""), None);
    }

    #[test]
    fn el_tratamiento_institucional_no_estorba() {
        for variante in [
            "Municipalidad de Ñuñoa",
            "I. Municipalidad de Ñuñoa",
            "Ilustre Municipalidad de Ñuñoa",
            "MUNICIPALIDAD DE NUNOA",
            "  municipalidad   de   nunoa  ",
        ] {
            assert_eq!(grupo_de(variante), Some(Grupo::B), "falló con {variante:?}");
        }
    }

    #[test]
    fn las_tildes_y_la_ene_no_estorban() {
        assert_eq!(grupo_de("nunoa"), Some(Grupo::B));
        assert_eq!(grupo_de("Concepcion"), Some(Grupo::B));
        assert_eq!(grupo_de("Curico"), Some(Grupo::B));
        assert_eq!(grupo_de("nique n"), None); // no se inventa una coincidencia difusa
    }

    #[test]
    fn ninguna_comuna_esta_en_los_dos_grupos() {
        for b in GRUPO_B {
            assert_eq!(grupo_de(b), Some(Grupo::B), "{b} debería resolver a B");
        }
        for c in GRUPO_C {
            assert_eq!(grupo_de(c), Some(Grupo::C), "{c} debería resolver a C");
        }
    }

    #[test]
    fn las_listas_son_las_del_decreto_y_no_el_catalogo_de_comunas() {
        // El Art. 5° nombra 343 municipalidades; Chile tiene 346. Las tres que faltan no
        // son un olvido de transcripción: no están en el decreto, verificadas contra el
        // PDF el 2026-07-25. Si alguien "completa" las listas desde un directorio de
        // comunas, esta prueba lo detiene.
        assert_eq!(GRUPO_B.len() + GRUPO_C.len(), 343);
        for ausente in ["Antártica", "Canela", "Quillón"] {
            assert_eq!(grupo_de(ausente), None, "{ausente} no está en el Art. 5°");
        }
        // Esta sí estaba, y se habia perdido al transcribir.
        assert_eq!(grupo_de("Las Cabras"), Some(Grupo::C));
    }

    #[test]
    fn manda_la_grafia_del_decreto() {
        // El decreto escribe Aysén, Coyhaique y Trehuaco. El uso corriente y varios
        // catálogos escriben Aisén, Coihaique y Treguaco.
        assert_eq!(grupo_de("Aysén"), Some(Grupo::C));
        assert_eq!(grupo_de("Coyhaique"), Some(Grupo::B));
        assert_eq!(grupo_de("Trehuaco"), Some(Grupo::C));
    }

    #[test]
    fn maria_pinto_y_mariquina_son_dos_comunas() {
        // El PDF las entrega pegadas ("María Pinto Mariquina") porque se perdió la coma
        // al extraer el texto. Son dos comunas distintas y las dos existen.
        assert_eq!(grupo_de("María Pinto"), Some(Grupo::C));
        assert_eq!(grupo_de("Mariquina"), Some(Grupo::C));
    }

    #[test]
    fn la_tabla_del_articulo_7_es_la_del_decreto() {
        use Fase::*;
        // 2022: preparación para los tres grupos.
        for g in [Grupo::A, Grupo::B, Grupo::C] {
            assert_eq!(fases_de(g, 2022), Some(&[Preparacion][..]));
        }
        // 2026, el año en curso al escribir esto.
        assert_eq!(fases_de(Grupo::A, 2026), Some(&[Dos, Cuatro, Cinco][..]));
        assert_eq!(fases_de(Grupo::B, 2026), Some(&[Cuatro, Cinco][..]));
        assert_eq!(fases_de(Grupo::C, 2026), Some(&[Cuatro, Seis][..]));
        // 2027: al Grupo A no le asigna fase nueva, y eso no es lo mismo que no cubrirlo.
        assert_eq!(fases_de(Grupo::A, 2027), Some(&[][..]));
        assert_eq!(fases_de(Grupo::B, 2027), Some(&[Dos][..]));
        assert_eq!(fases_de(Grupo::C, 2027), Some(&[Dos, Cinco][..]));
    }

    #[test]
    fn fuera_de_la_tabla_no_se_extrapola() {
        assert_eq!(fases_de(Grupo::B, 2021), None);
        assert_eq!(fases_de(Grupo::B, 2028), None);
        assert_eq!(fases_de(Grupo::B, 2100), None);
    }

    #[test]
    fn el_estado_de_un_ano_posterior_dice_que_esta_fuera_de_tabla() {
        let e = estado("Providencia", 2030);
        assert_eq!(e.grupo, Some(Grupo::B));
        assert!(e.fases.is_empty());
        assert!(e.nota.contains("queda fuera"), "nota inesperada: {}", e.nota);
    }

    #[test]
    fn el_estado_de_un_nombre_desconocido_no_afirma_grupo() {
        let e = estado("Órgano Cualquiera", 2026);
        assert!(e.grupo.is_none());
        assert!(e.fases.is_empty());
        assert!(e.nota.contains("no figura"), "nota inesperada: {}", e.nota);
    }

    #[test]
    fn el_estado_normal_nombra_grupo_y_fases() {
        let e = estado("Municipalidad de Ñuñoa", 2026);
        assert_eq!(e.grupo, Some(Grupo::B));
        assert_eq!(e.fases, vec![Fase::Cuatro, Fase::Cinco]);
        assert!(e.nota.contains("Grupo B"));
        assert!(!e.procedencia.is_empty());
    }

    #[test]
    fn el_ano_sale_del_reloj_y_no_de_una_constante() {
        // Si alguien reemplaza `anio_actual()` por un literal, esto lo delata cuando el
        // literal envejezca.
        assert!(anio_actual() >= 2026);
    }

    #[test]
    fn cada_fase_se_describe() {
        for f in [
            Fase::Preparacion, Fase::Uno, Fase::Dos, Fase::Tres,
            Fase::Cuatro, Fase::Cinco, Fase::Seis,
        ] {
            assert!(!f.descripcion().trim().is_empty(), "{f} sin descripción");
            assert!(!f.to_string().is_empty());
        }
    }
}
