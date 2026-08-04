# corpus_defensa

Normativa del sector Defensa que se agrega al corpus nacional para construir las bases
institucionales del Asistente. Los PDF no se versionan aquí: ya están en `docs/` del
repositorio, que es su lugar canónico. Este archivo existe para que la base se pueda
reconstruir sabiendo exactamente qué entró.

## Cómo se reconstruye

La base institucional **no** es el corpus nacional completo. La mitad de ese corpus es
normativa exclusivamente municipal, y con ella dentro una pregunta sobre personal puede
recuperar el Estatuto Administrativo para Funcionarios Municipales y citárselo a un
organismo de las Fuerzas Armadas. Así que se arma en dos pasadas: primero las leyes
transversales, después la normativa de Defensa de esta carpeta.

Leyes nacionales que sí entran, copiadas desde `corpus/`:

```
constitucion_politica_1980.txt
ley_19880_procedimientos_administrativos.txt
ley_20285_transparencia.txt
ley_21180_transformacion_digital.txt
ley_21663_ciberseguridad.txt
ley_19886_compras_publicas.txt
ley_20730_lobby.txt
```

Quedan fuera las once municipales (ley orgánica de municipalidades, rentas municipales,
estatuto de funcionarios municipales, juzgados de policía local, juntas de vecinos,
estatuto docente, expendio de alcoholes, consejo de seguridad comunal, educación pública,
atención primaria de salud y urbanismo) y otras cuatro sin relación con la materia
(código del trabajo, clasificaciones presupuestarias, royalty minero y participación
ciudadana).

```powershell
# Desde assistant\backend\, con las siete leyes de arriba copiadas a <general>\
..\.venv\Scripts\python.exe ingest.py --corpus-dir <general> --db-dir db_<slug> --reset
..\.venv\Scripts\python.exe ingest.py --corpus-dir corpus_defensa --db-dir db_<slug>
```

Ninguna de las dos instituciones tiene hoy estatuto orgánico ni de personal en el corpus:
la Ley N° 18.948 aparece citada en los Vistos del reglamento de 2025 pero no está
indexada. Una pregunta sobre personal no tiene respuesta en esta base, que es preferible
a tener una equivocada.

El `<slug>` lo deriva `rag._municipio_slug` del nombre de la institución, y las variantes
con y sin tilde caen en el mismo valor:

| Institución | Carpeta |
|---|---|
| Ejército de Chile | `db_ejercito-de-chile` |
| Fuerza Aérea de Chile | `db_fuerza-aerea-de-chile` |

## Documentos

Los cuatro se descargaron de su fuente primaria y se verificaron leyendo su propia
primera página. Ninguno viene de prensa ni de un repositorio de terceros.

| Archivo (en `docs/`) | Qué es | Fuente | SHA256 |
|---|---|---|---|
| `Decreto-2_31-DIC-2025_Reglamento-Ciberseguridad-Defensa-Nacional.pdf` | Decreto Núm. 2 de la Subsecretaría de Defensa, Ministerio de Defensa Nacional, dictado el 23 de mayo de 2025. Aprueba el Reglamento de Ciberseguridad de la Defensa Nacional. Diario Oficial Núm. 44.337, 31 de diciembre de 2025, CVE 2748664 | `https://www.doe.cl/alerta/31122025/2748664` | `63ef9e98...61cf6bb` |
| `Decreto-3_09-MAR-2018_Politica-de-Ciberdefensa.pdf` | Decreto Núm. 3 de 2017 del Ministerio de Defensa Nacional, de 9 de noviembre de 2017. Aprueba Política de Ciberdefensa. Diario Oficial Núm. 42.003, 9 de marzo de 2018, CVE 1363153 | `https://www.diariooficial.interior.gob.cl/publicaciones/2018/03/09/42003/01/1363153.pdf` | `2cc7b4c5...84d7f6c9b` |
| `SSFFAA-Politica-General-de-Seguridad-de-la-Informacion_17-OCT-2025.pdf` | Resolución Exenta N° 8028, 17 de octubre de 2025. Aprueba la Política General de Seguridad de la Información de la Subsecretaría para las Fuerzas Armadas | `https://www.ssffaa.cl/wp-content/uploads/2025/10/Politica-General-de-Seguridad-de-la-Informacion_web.pdf` | `907f2157...5323f9b3e` |
| `Politica-de-Defensa-Nacional-de-Chile-2020.pdf` | Política de Defensa Nacional de Chile, edición 2020, aprobada por Decreto Supremo N° 4 de 4 de diciembre de 2020 | `https://www.defensa.cl/wp-content/uploads/2023/06/POL%C3%8DTICA-DE-DEFENSA-NACIONAL-DE-CHILE-2020.pdf` | `00f4d978...7ee37a73a` |

## Por qué estos y no los reglamentos institucionales

El marco normativo público de la Fuerza Aérea que se puede alcanzar por URL trata de
documentación, educación, disciplina, heráldica y gestión de la información de seguridad
operacional. Nada de eso es ciberseguridad, y agregarlo diluiría la recuperación con
material ajeno a la pregunta que el producto responde. Del Ejército no se encontró
normativa institucional descargable desde fuente primaria: lo disponible está en sitios
de terceros, que esta carpeta no acepta.

Los portales de transparencia de ambas instituciones responden 403 a una descarga
programática de sus páginas HTML, aunque sirven los PDF cuya URL ya se conoce, así que
el índice no se puede enumerar desde aquí. Si hace falta ampliar el corpus, los archivos
los aporta Felipe.

Dos documentos de la Fuerza Aérea alcanzables por URL llevan en su encabezado las marcas
"RESERVADO" y "CLASIFICACIÓN". No se incorporan sin una decisión explícita.
