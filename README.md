# MuniANCI

Escáner de cumplimiento de ciberseguridad para organismos del Estado chileno, alineado a **Ley 21.663 (Marco de Ciberseguridad)** y las Instrucciones Generales de la ANCI.

Combina escaneo activo de red con un cuestionario declarativo para producir un **informe de brechas en PDF** y un **reporte JSON listo para CSIRT Chile**.

---

## Aviso legal

El uso de esta herramienta en redes de organismos del Estado requiere:

1. Inscripción previa en el registro de la ANCI ([portal.anci.gob.cl](https://portal.anci.gob.cl))
2. Notificación previa a la ANCI del acceso
3. Reporte de vulnerabilidades al responsable del sistema y a la ANCI

Cumplidas estas condiciones, el acceso queda amparado por el **safe harbor del Art. 2° Ley 21.459**. Sin ellas, puede constituir acceso ilícito.

---

## Requisitos

- Rust 1.78+ (`rustup update stable`)
- Windows 10/11 o Linux (Ubuntu 22.04+)
- Privilegios de administrador local recomendados (BitLocker, WMI)

---

## Instalación

```bash
git clone https://github.com/fcarvajalbrown/MuniANCI
cd MuniANCI
cargo build --release
```

El binario queda en `target/release/muniani-cli` (o `.exe` en Windows).

---

## Uso

```bash
# Escaneo local con cuestionario interactivo
muniani-cli --name "Municipalidad de X" --tier pse --scope local

# Escaneo LAN sin cuestionario (todos los controles declarativos = no cumplido)
muniani-cli --name "Municipalidad de X" --tier oiv --scope lan --no-questionnaire

# Salida personalizada
muniani-cli --name "..." --pdf informe.pdf --json csirt.json
```

### Flags

| Flag | Default | Descripción |
|------|---------|-------------|
| `--name` | `"Institución sin nombre"` | Nombre de la institución |
| `--tier` | `pse` | Clasificación: `oiv`, `pse`, `unclassified` |
| `--scope` | `local` | Alcance: `local`, `lan` |
| `--pdf` | `informe_brechas.pdf` | Ruta del informe PDF |
| `--json` | `csirt_report.json` | Ruta del reporte JSON |
| `--no-questionnaire` | — | Omite el cuestionario declarativo |

---

## Controles evaluados

### Objetivos (escaneados automáticamente)
- Shares anónimos SMB/NFS/WebDAV
- Admin shares expuestos (C$, ADMIN$, IPC$)
- Firewall desactivado
- Protocolos en claro (Telnet, FTP)
- TLS 1.0/1.1/SSLv3 activo
- Certificados vencidos o autofirmados
- Sistema operativo en EOL
- Software en EOL
- Disco fijo sin cifrado en reposo (BitLocker/LUKS)
- Cloud sync activo (OneDrive, Dropbox, etc.)
- Sin agente de backup detectado

### Declarativos (cuestionario interactivo)
- Inscripción en plataforma ANCI (IG N°1)
- Delegado de Ciberseguridad designado (Art. 8° lit. i)
- SGSI implementado (Art. 8° lit. a)
- Registro de acciones SGSI (Art. 8° lit. b)
- Plan de continuidad elaborado (Art. 8° lit. c)
- Plan de continuidad certificado (Art. 28°)
- Programa de capacitación continua (Art. 8° lit. h)

---

## Arquitectura

```
muniani/
├── core/   # Library crate — toda la lógica de escaneo y cumplimiento
├── cli/    # Binary crate — interfaz de línea de comandos
└── gui/    # Binary crate — Tauri 2 shell (v0.2, pendiente)
```

Ver [CHANGELOG.md](CHANGELOG.md) para historial de versiones.

---

## Marco normativo

| Norma | Descripción |
|-------|-------------|
| Ley 21.663 (DO 08/04/2024) | Ley Marco de Ciberseguridad |
| Ley 21.459 (DO 20/06/2022) | Delitos informáticos / Convenio de Budapest |
| DFL N°1/2024 | Entrada en vigor Art. 5°, 8°, 9° (01/03/2025) |
| DS N°295/2024 | Reglamento de Reporte de Incidentes |
| IG N°1 ANCI (jun 2025) | Inscripción plataforma reporte |
| IG N°2, 3, 4 ANCI (dic 2025) | Reporte complementario, delegado, contención |

---

## Licencia

MIT — Felipe Carvajal Brown Software