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
- Node.js 20+ y npm 10+ (solo para compilar la GUI)
- Windows 10/11 o Linux (Ubuntu 22.04+)
- WebView2 Runtime instalado (Windows — incluido en Win11, descargable gratis en Win10)
- Privilegios de administrador local recomendados (BitLocker, WMI)

---

## Instalación

```powershell
git clone https://github.com/fcarvajalbrown/MuniANCI
cd MuniANCI
cargo build --release -p muniani-cli
```

El binario CLI queda en `target\release\muniani-cli.exe`.

Para compilar la GUI ver sección **Compilación por cliente** más abajo.

---

## Uso — CLI

```powershell
# Escaneo local con cuestionario interactivo
muniani-cli --name "Municipalidad de X" --tier pse --scope local

# Escaneo LAN sin cuestionario (todos los controles declarativos = no cumplido)
muniani-cli --name "Municipalidad de X" --tier oiv --scope lan --no-questionnaire

# Salida personalizada
muniani-cli --name "..." --pdf informe.pdf --json csirt.json
```

### Flags CLI

| Flag | Default | Descripción |
|------|---------|-------------|
| `--name` | `"Institución sin nombre"` | Nombre de la institución |
| `--tier` | `pse` | Clasificación: `oiv`, `pse`, `unclassified` |
| `--scope` | `local` | Alcance: `local`, `lan` |
| `--pdf` | `informe_brechas.pdf` | Ruta del informe PDF |
| `--json` | `csirt_report.json` | Ruta del reporte JSON |
| `--no-questionnaire` | — | Omite el cuestionario declarativo |

---

## Uso — GUI (v0.2)

La GUI (`muniani-gui`) es una aplicación de escritorio Tauri 2 con dos vistas:

- **Vista Municipal** — resumen ejecutivo en español formal, escala de multas UTM, aviso de obligación CSIRT
- **Vista Técnica (TI)** — tabla completa de brechas con evidencia, terminal de log en tiempo real, exportación PDF/JSON

### Compilación por cliente

El nombre de la institución y el tier se compilan directamente en el binario — no hay archivo de configuración editable por el usuario final.

```powershell
# Desde gui\
$env:MUNIANI_INSTITUTION = "Municipalidad de Chillán"
$env:MUNIANI_TIER        = "pse"
cargo tauri build
```

El instalador queda en `target\release\bundle\`.

#### Variables de entorno de compilación

| Variable | Valores válidos | Descripción |
|----------|----------------|-------------|
| `MUNIANI_INSTITUTION` | Cualquier string | Nombre de la institución cliente |
| `MUNIANI_TIER` | `oiv`, `pse`, `unclassified` | Clasificación bajo Ley 21.663 |

> Si `MUNIANI_INSTITUTION` no se define, el binario mostrará `"Municipalidad de Prueba"`.
> Si `MUNIANI_TIER` no se define, el binario usará `pse` por defecto.

### Ejecución en modo desarrollo

```powershell
cd gui\frontend
npm install
cd ..
cargo tauri dev
```

---

## Controles evaluados

### Objetivos (escaneados automáticamente)

| Control | Severidad | Tier | Fundamento |
|---------|-----------|------|------------|
| Shares anónimos SMB/NFS/WebDAV | Crítico | Todos | Art. 8° lit. e); IG N°4 |
| Admin shares expuestos (C$, ADMIN$) | Crítico | Todos | Art. 8° lit. e); IG N°4 |
| Firewall desactivado | Crítico | Todos | Art. 8° lit. e); IG N°4 |
| Protocolos en claro (Telnet/FTP) | Crítico | Todos | Art. 8° lit. e); IG N°4 |
| TLS 1.0/1.1/SSLv3 activo | Crítico | OIV+PSE | Art. 8° lit. a); NIST SP 800-52 |
| Sistema operativo en EOL | Crítico | Todos | Art. 8° lit. a) y d) |
| Certificado vencido o autofirmado | Alto | OIV+PSE | Art. 8° lit. a) |
| Software en EOL | Alto | Todos | Art. 8° lit. a) y d) |
| Disco fijo sin cifrado (BitLocker/LUKS) | Alto | OIV | Art. 8° lit. a); ISO 27001 A.10.1 |
| Cloud sync activo | Alto | OIV | Art. 8° lit. a) |
| Sin agente de backup detectado | Alto | OIV+PSE | Art. 8° lit. b) |

### Declarativos (cuestionario interactivo)

| Control | Severidad | Tier | Fundamento |
|---------|-----------|------|------------|
| Inscripción en plataforma ANCI | Crítico | OIV+PSE | IG N°1 ANCI (jun 2025) |
| Delegado de Ciberseguridad designado | Alto | OIV | Art. 8° lit. i); IG N°3 |
| SGSI implementado | Crítico | OIV | Art. 8° lit. a) |
| Registro de acciones SGSI | Alto | OIV | Art. 8° lit. b) |
| Plan de continuidad elaborado | Alto | OIV | Art. 8° lit. c) |
| Plan de continuidad certificado | Alto | OIV | Art. 8° lit. c) + Art. 28° |
| Programa de capacitación continua | Medio | OIV | Art. 8° lit. h) |

---

## Base de datos EOL (v0.2)

La herramienta incluye una base de datos estática de fin de vida compilada de [endoflife.date](https://endoflife.date) (snapshot marzo 2026), embebida en el binario. Cubre 38 productos incluyendo Windows, Office, SQL Server, .NET, Python, Node.js, PHP, MySQL, PostgreSQL, Apache, nginx, OpenSSL y más.

> La base EOL se actualiza en cada release. Verificar versiones actualizadas en [endoflife.date](https://endoflife.date).

---

## Arquitectura

```
muniani/
├── core/        # Library crate — lógica de escaneo, cumplimiento y enriquecimiento EOL
│   └── src/
│       └── data/
│           └── eol_db.json   # Base EOL embebida (include_str!)
├── cli/         # Binary crate — interfaz de línea de comandos
└── gui/         # Binary crate — Tauri 2 desktop app (v0.2)
    └── frontend/ # React/TypeScript/Vite
```

Ver [CHANGELOG.md](CHANGELOG.md) para historial de versiones.

---

## Nota sobre firma de código

El ejecutable de MuniANCI **no está firmado digitalmente** en esta versión. Windows Defender y soluciones AV empresariales (McAfee, Defender ATP) mostrarán una advertencia al instalar.

La firma con certificado Authenticode (DigiCert/Sectigo, ~USD 200-400/año) está pendiente para la versión de distribución municipal. Hasta entonces, el equipo de TI del cliente debe aprobar manualmente la ejecución.

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