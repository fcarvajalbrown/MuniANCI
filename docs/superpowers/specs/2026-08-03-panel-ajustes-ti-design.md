# IT settings panel (cog) — design

Date: 2026-08-03
Status: approved, not yet implemented
Target milestone: v0.8.0

## Why

Everything IT can tune already exists as `munianci.config.json` (`core/src/config.rs`),
but the only way to edit it is a text editor, and nobody edits a file they do not know
exists. The institution name is worse than undiscoverable: it is compiled into the
binary via `MUNIANI_INSTITUTION`, so a wrong name costs a rebuild and a reinstall.

A cog in the header, behind a password, gives municipal and institutional IT one place
to change what belongs to them, and makes the identity fixable without a new installer.

The immediate forcing function is two demos to the Fuerza Aerea de Chile and the
Ejercito de Chile, two hours apart. The product currently defaults to
`Municipalidad de Providencia`, which cannot be what either audience sees.

## Decisions taken

All of these were decided by Felipe, and each gets its own ADR.

| Decision | Choice |
| --- | --- |
| What the cog contains | Identidad, Plazos e historico, Red y monitoreo, Informe |
| Password source | Compiled per client at build, rotatable in-panel |
| Rename blast radius | Everything, Asistente included |
| UI shape | Anchored dropdown with collapsible sections |
| Cog placement | Header, always visible |
| Stale scan handling | Warn, keep the result |
| Panel extras | Change password, restore defaults, show config origin, open the file |
| Default institution | Neutral placeholder, not a real client |
| Default tier | `pse` |
| De-municipalisation | Deferred to its own roadmap milestone |

## Architecture

### Config model — `core/src/config.rs`

A new section, following the conventions the file already sets: `#[serde(default)]`,
forward-compatible, BOM-tolerant.

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IdentidadConfig {
    pub institucion: Option<String>,
    pub tier: Option<String>,
}
```

`Option` rather than a `String` default, because absent and empty must stay
distinguishable: absent means "fall through to the compiled value", empty means IT
cleared the field and we refuse to save it.

Resolution order for the institution, first hit wins:

1. `munianci.config.json` -> `identidad.institucion`
2. compiled `MUNIANI_INSTITUTION`
3. `DEFAULT_INSTITUTION`

Same three steps for the tier, against `MUNIANI_TIER` and `DEFAULT_TIER`.

Two default changes:

- `DEFAULT_INSTITUTION`: `"Municipalidad de Providencia"` becomes `"Organismo del Estado"`.
  A real client must not be the fallback for every un-branded build; that is the
  problem being fixed, and the next demo is to somebody else.
- `DEFAULT_TIER`: stays `"pse"`. Art. 1 inc. 2 of Ley 21.663 places the Fuerzas
  Armadas inside the Administracion del Estado for the law's purposes, and Art. 4
  inc. 2 makes services provided by organs of the Administracion del Estado essential
  by that fact alone. So a State organ with no ANCI resolution is a prestador de
  servicios esenciales. `Tier::Unclassified` would be wrong: it switches off
  `requires_csirt_report` for every gap (`compliance_engine.rs:536`), telling a State
  organ it has no reporting duty. `Tier::Oiv` would assert a calificacion that only
  the Agencia can confer by resolucion fundada (Arts. 5 and 6).

`Config` gains a writer:

```rust
pub fn guardar(&self, path: &Path) -> std::io::Result<()>
```

Atomic: serialize to `<path>.tmp`, then rename over the target, so a crash mid-write
cannot leave IT with a truncated config. The `_ayuda` header is preserved on save —
a round trip through the panel must not strip the documentation that makes the file
editable by hand.

`Config::ejemplo()` gains the identidad block and its help lines, so
`munianci --escribir-config` keeps documenting the whole surface.

### Password — `core/src/ti.rs` (new)

Argon2id, via the `argon2` crate. One new Rust dependency; nothing else in the
workspace hashes passwords, and hand-rolling this would be worse.

- Compiled hash: `option_env!("MUNIANI_ADMIN_HASH")`, a PHC string, set at build time
  next to `MUNIANI_INSTITUTION` and `MUNIANI_TIER`.
- Override: `%LOCALAPPDATA%\MuniANCI\ti-password.hash`, written when IT rotates the
  password. It wins over the compiled hash.
- Recovery: deleting the override file restores the build password. Documented in the
  README for IT.
- No compiled hash and no override (dev and un-branded builds): the first cog press
  asks IT to set a password, which becomes the override. No known default password
  ships.
- Failed attempts back off in memory: 1s, 2s, 4s, capped at 30s, reset on success and
  on app restart. Not persisted.

**Stated threat model.** This is an accident guard. It stops a municipal worker from
wandering into settings and changing scan deadlines or the institution name. It does
not stop anyone with file access: `munianci.config.json` stays editable with Notepad,
by design, and the panel and the README say so in as many words. Signing the config
was considered and rejected, because it would break the standing promise that the file
is hand-editable and would turn a lost password into a lockout from IT's own config.

### Host commands — `gui/src/commands/ajustes.rs` (new)

| Command | Returns | Notes |
| --- | --- | --- |
| `ti_estado` | `{ configurada, origen }` | Is a password set; resolved config path or "valores por defecto" |
| `ti_desbloquear` | `bool` | Verifies and opens a session in Rust state |
| `ti_leer` | `Config` | Session required |
| `ti_guardar` | `{ requiereReinicioAsistente, afectaInforme }` | Session required |
| `ti_cambiar_password` | `bool` | Current password required |
| `ti_restaurar_defectos` | `Config` | Per section |
| `ti_abrir_archivo` | `()` | Opens the config in the system editor |
| `asistente_reiniciar` | `()` | `shutdown()` then `start()` |

The unlock returns a session held in managed Rust state, not a token the webview keeps.
The password never lives in the frontend beyond the keystrokes in the input.

`assistant.rs` changes at the `MUNIGPT_MUNICIPIO` decision (currently line 211): it
reads `branding::institution_override()`, which is compiled-only. It must read the
resolved runtime identity instead, so a rename reaches both the backend prompt
personalization and `db_<slug>` selection.

When the institution changes and no `db_<slug>` exists for the new value, the backend
falls back to the national corpus in `db/`. The Asistente tab shows that fallback
plainly rather than implying an institutional corpus that was never shipped.

### Frontend — `gui/frontend/src/components/AjustesTI.tsx` (new)

Cog button top-right in `app-header`, on every tab. Anchored dropdown panel, closing on
Escape and on outside click, with focus trapped while open.

Locked state: password field, unlock button, and the failed-attempt backoff surfaced as
a disabled button with a countdown rather than a silent rejection.

Unlocked state: an accordion, one section open at a time, scrollable.

- **Identidad** — institution (text), tier (select). Saving a changed institution
  confirms first: it restarts the Asistente, chat history in that tab is lost, and the
  backend can take up to 180 seconds to report ready again.
- **Plazos e historico** — `poam.plazo_dias_*`, `historico.*`. The panel repeats what
  the config file already says: these are not legal deadlines.
- **Red y monitoreo** — `red.*`, `monitoreo.*`. The `arp_pps` field carries the Dynamic
  ARP Inspection warning inline, not in a tooltip. It is the one setting here that can
  take the machine off the network.
- **Informe** — paper sizes, the four palette colours.

Footer: Guardar, Cancelar, Cambiar contrasena, Restaurar valores por defecto, Abrir el
archivo de configuracion, and a line showing where the effective config came from.

### Stale results

`App.tsx` keeps a config generation counter. When `ti_guardar` reports `afectaInforme`
and a `result` is on screen, a banner says the configuration changed after this scan and
offers "Escanear de nuevo". The old result stays readable. The point is that nobody
exports a PDF whose stated deadlines contradict the configuration section of that same
PDF.

`afectaInforme` is computed per config block, not per UI section: true for `identidad`,
`poam` and `red`, false for `informe`, `historico` and `monitoreo`. So changing the
rescan schedule, which shares a UI section with the network settings, does not mark a
scan stale, and neither does a colour or paper change.

## Testing

Rust, in the existing style of `config.rs`:

- identidad resolution order across all three sources, including empty-string rejection
- a config written by the panel round-trips through the loader with `_ayuda` intact
- an old config with no identidad section still loads
- atomic write leaves no partial file when the rename target exists
- Argon2id verify accepts the right password and rejects the wrong one
- the override hash wins over the compiled one, and deleting it falls back
- `ti_leer` and `ti_guardar` refuse without a session

Frontend behaviour (accordion, focus trap, backoff countdown) is verified by hand in the
running app; there is no frontend test harness in this repo and this design does not
add one.

## Out of scope

- **De-municipalisation.** Neutralising "Vista Municipal", "municipalidad" and the
  report and Asistente wording is its own roadmap milestone.
- **CSIRT-DN routing.** Under the Reglamento de Ciberseguridad de la Defensa Nacional
  (Decreto N 2, Ministerio de Defensa Nacional, Diario Oficial 31-DIC-2025, to be
  verified against the PDF) a defence organism reports to CSIRT-DN, which relays to
  ANCI. The report and the ANCI JSON currently address CSIRT Nacional. Its own roadmap
  milestone.
- **Renaming the product.** MuniANCI stays MuniANCI.
- **CSP port bug.** `tauri.conf.json` pins `connect-src` to `http://127.0.0.1:8000`
  while `puerto_utilizable` can pick another port, which the CSP would then block.
  Real, pre-existing, unrelated to this feature.

## Companion work

Not part of the panel, agreed in the same session:

1. Fetch into `docs/`: the Reglamento de Ciberseguridad de la Defensa Nacional, the
   Politica General de Seguridad de la Informacion (ssffaa.cl, October 2025), the
   Politica Nacional de Ciberdefensa and the Politica de Defensa Nacional 2020, and the
   public FACH reglamentos from its marco normativo. `.gitattributes` already marks
   `*.pdf` as binary; the round trip still gets proven per the repo rule.
2. Two ROADMAP milestones: desmunicipalizar, and CSIRT-DN routing.
3. ADRs for the decisions in the table above, and a `CLAUDE.md` correction: the line
   saying the client identity is not IT configuration is reversed by this design.
