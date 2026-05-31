# philosophical machine — build plan

> a program that perceives the world through a camera, reduces what it sees to
> geometric primitives, composes them into relational graphs, elaborates them
> through fractal deepening across repeated encounters, and remembers everything
> it has understood — running entirely offline on a debian 13 terminal.

---

## philosophical grounding

the machine enacts three acts per cycle, mirroring husserlian intentionality:

- **comprehension** — directed perception: the camera looks, finds closed forms,
  classifies them against memory
- **action** — the found form is drawn as SVG, deepened fractally if already known
- **intellection** — the machine reflects on its own output, stores the result,
  updates its model

primitives are **conclusions**, not axioms. the machine begins with no vocabulary.
shapes emerge from point topology. circles are banned — they are geometrically
exhausted, offering no vertices to subdivide and no edges to elaborate.
a polygon approaching circularity is more honest: it preserves incompleteness.

---

## system overview

```
camera frame
    │
    ▼
perceive.py ──── edge detection, contour extraction, saliency scoring
    │
    ▼
recognize.py ─── contours → closed polygons → classified primitives
    │             (rejects circles, degenerate shapes)
    ▼
compose.py ────── primitives → spatial graph (who touches whom, how)
    │
    ▼
memory.py ─────── look up known forms, retrieve generation number
    │
    ▼
elaborate.py ───── primitive/graph + generation → fractally deepened SVG
    │
    ▼
plot.py ─────────── write SVG to output/ (plotter stub for now)
    │
    ▼
[contemplate — fixed pause]
    │
    ▼
perceive.py ─────── re-perceive own output (intellection)
    │
    ▼
memory.py ─────────── store result, increment generation, append log
    │
    ▼
[erase — clear output/]
    │
    ▼
repeat
```

---

## directory structure

```
machine/
  main.py              # textual TUI app + async cycle orchestrator
  perceive.py          # frame capture, edge detection, contour extraction
  recognize.py         # polygon classification, circle rejection, hashing
  compose.py           # spatial graph construction between primitives
  elaborate.py         # SVG generation, fractal elaboration by generation
  memory.py            # filesystem read/write, hash lookup, log append
  plot.py              # SVG → output/ (plotter interface stub)

  tui/
    __init__.py
    panels.py          # textual widget definitions (one per panel)
    ascii.py           # contour/SVG → character grid renderer

  memory/
    primitives/
      line/
      triangle/
      quad/
      pentagon/
      hexagon/
      n-gon/           # vertex count > 6, stored as "n7", "n8" etc.
    graphs/            # named by sorted primitive hash combination
    log.jsonl          # append-only, one JSON object per line

  output/              # working SVG for current cycle, cleared after
  observations/        # raw frame JPEGs, timestamped
  config.toml          # all thresholds and parameters, no magic numbers
```

---

## module specifications

### `config.toml`

all tunable parameters in one place. no magic numbers in source.

```toml
[perception]
camera_index       = 0
frame_width        = 640
frame_height       = 480
gaussian_blur      = 5
canny_low          = 50
canny_high         = 150
min_contour_area   = 500       # px² — filters dust
saliency_threshold = 0.3       # below this, skip frame

[recognition]
approx_epsilon     = 0.03      # fraction of arc length for approxPolyDP
circularity_limit  = 0.75      # shapes above this are rejected as circles
min_vertices       = 2         # minimum polygon vertices (line = 2)
max_vertices       = 12        # above this, too complex to classify cleanly

[composition]
proximity_threshold = 0.15     # fraction of frame width — max distance for "near"
shared_edge_tolerance = 0.05   # endpoint distance to count as shared edge

[elaboration]
max_generation     = 6         # fractal depth ceiling
contemplate_secs   = 30        # pause after plotting before intellection

[memory]
memory_dir         = "memory/"
output_dir         = "output/"
observations_dir   = "observations/"
log_file           = "memory/log.jsonl"
```

---

### `perceive.py`

**inputs:** camera index or image file path (for testing)
**outputs:** list of raw contours, saliency score float, ASCII preview string

**functions:**

`capture_frame(source) → np.ndarray`
accepts int (camera index) or str (file path). returns BGR frame.
raises `PerceptionError` if source unavailable.

`detect_edges(frame, config) → np.ndarray`
grayscale → gaussian blur → canny. returns edge map.

`extract_contours(edge_map, config) → list[np.ndarray]`
findContours with RETR_EXTERNAL, CHAIN_APPROX_SIMPLE.
filters by minimum area from config.
returns list of raw contours sorted by area descending.

`score_saliency(contours, frame_shape) → float`
edge pixel density / contour count.
penalizes chaos (many tiny contours), rewards clean structure.
returns float 0–1.

`contours_to_ascii(contours, frame_shape, cols=40, rows=20) → str`
bins contour points into character grid.
uses `█` for strong edges, `░` for weaker, ` ` for empty.
returns multiline string, ready for TUI display.

`save_observation(frame, observations_dir) → str`
writes timestamped JPEG to observations/.
returns filename.

---

### `recognize.py`

**inputs:** list of raw contours, config
**outputs:** list of Primitive objects

**Primitive dataclass:**
```python
@dataclass
class Primitive:
    type: str              # "line", "triangle", "quad", "pentagon",
                           # "hexagon", "n7", "n8" ...
    vertices: int
    coords: np.ndarray     # normalized 0–1, centroid at origin
    bbox: tuple            # (x, y, w, h) in original frame pixels
    hash: str              # stable identifier for memory lookup
    circularity: float     # for diagnostics
    aspect_ratio: float
```

**functions:**

`classify_contours(contours, config) → list[Primitive]`
main entry point. runs the full pipeline per contour.

`approximate_polygon(contour, epsilon_fraction) → np.ndarray`
wraps approxPolyDP. returns simplified vertex array.

`compute_circularity(contour) → float`
4π * area / perimeter². rejects above config threshold.

`classify_by_vertices(n) → str`
2 → "line", 3 → "triangle", 4 → "quad", 5 → "pentagon",
6 → "hexagon", n > 6 → f"n{n}"

`normalize_coords(vertices, frame_shape) → np.ndarray`
scales to 0–1 space. centers on centroid.

`compute_hash(primitive) → str`
combines: vertex count + quantized aspect ratio (4 bins)
+ quantized internal angle variance (3 bins).
produces short stable string. same shape across observations
→ same hash. format: `t3_r2_a1` (type, ratio bin, angle bin).

`reject_circle(circularity, limit) → bool`
returns True if shape should be discarded.

---

### `compose.py`

**inputs:** list of Primitive objects, config
**outputs:** Graph object

**Graph dataclass:**
```python
@dataclass
class Graph:
    nodes: list[Primitive]
    edges: list[Edge]
    hash: str              # sorted combination of node hashes

@dataclass
class Edge:
    a: int                 # index into nodes
    b: int                 # index into nodes
    relation: str          # "shared_vertex", "shared_edge",
                           # "contained", "near"
```

**spatial relations:**

`shared_vertex` — two primitives share a vertex within tolerance
`shared_edge` — two primitives share two adjacent vertices
`contained` — one bounding box fully inside another
`near` — centroids within proximity threshold, no other relation

**functions:**

`build_graph(primitives, config) → Graph`
main entry point. computes all pairwise relations.
builds edge list. computes graph hash.

`compute_relation(a, b, config) → str | None`
returns strongest relation between two primitives or None.
priority: shared_edge > shared_vertex > contained > near.

`graph_hash(primitives) → str`
sorts primitive hashes, joins with "+".
`"t3_r2_a1+t4_r1_a0"` — stable, order-independent.

**note:** a single primitive with no relations still produces
a valid graph with one node and no edges. this is the base case —
the machine encountering a form in isolation.

---

### `memory.py`

**inputs:** graph or primitive hash, generation, SVG path, observation path
**outputs:** MemoryEntry objects, filesystem writes

**MemoryEntry dataclass:**
```python
@dataclass
class MemoryEntry:
    hash: str
    label: str             # human-readable: "triangle", "quad+triangle"
    generation: int
    count: int             # total observations
    last_seen: str         # ISO timestamp
    svgs: list[str]        # relative paths to generated SVGs by generation
    observations: list[str] # relative paths to observation JPEGs
    status: str            # "new", "deepening", "exhausted"
```

**filesystem layout per entry:**
```
memory/primitives/triangle/t3_r2_a1/
  meta.json
  gen_0.svg
  gen_1.svg
  gen_2.svg
  observations/
    20240112_143022.jpg
    20240115_091540.jpg
```

**functions:**

`lookup(hash, memory_dir) → MemoryEntry | None`
reads meta.json if exists. returns None for new forms.

`store(entry, memory_dir)`
writes or updates meta.json.
copies current SVG into correct gen_N.svg slot.
copies observation JPEG into observations/.

`increment(entry) → MemoryEntry`
bumps generation. updates last_seen. marks exhausted if
generation >= config max_generation.

`append_log(event, log_file)`
appends one JSON line to log.jsonl.
event is a dict with keys: timestamp, phase, hash, label,
generation, note.

`reset(memory_dir)`
deletes entire memory/ tree and recreates empty structure.
logs the reset event first.

`list_all(memory_dir) → list[MemoryEntry]`
reads all meta.json files recursively.
used by TUI memory panel.

---

### `elaborate.py`

**inputs:** Graph object, MemoryEntry (for generation), config
**outputs:** SVG file written to output/

**elaboration strategy by generation:**

generation 0 — draw the primitive(s) as-is. plain polygon outlines.
if a graph: draw each primitive, then draw relation lines between them
(dashed for "near", solid for "shared_edge", dotted for "contained").

generation 1 — subdivide each edge once. midpoint displaced outward
by a fixed fraction (Koch-like). each primitive begins to elaborate.

generation 2+ — recursive subdivision to depth = generation.
displacement fraction decreases per level to prevent explosion.

generation = max — mark SVG with a small annotation glyph (⊠) indicating
exhaustion. the machine has nothing more to say about this form.

**no circles at any generation.** subdivision of a polygon never
produces arcs. all curves are approximated as many-sided polygons
if needed, but this should not arise organically.

**functions:**

`elaborate(graph, entry, config, output_path)`
main entry point. dispatches to correct generation strategy.
writes SVG via svgwrite.

`draw_primitive(dwg, primitive, generation, offset)`
draws one polygon, subdivided to correct depth.

`draw_relation(dwg, a, b, relation)`
draws the edge between two primitives according to relation type.

`subdivide_polygon(vertices, depth, displacement) → list[tuple]`
recursive Koch-like subdivision. returns flat vertex list.
displacement halves each level.

`svg_to_ascii(svg_path, cols=40, rows=20) → str`
rasterizes SVG paths into character grid for TUI preview.
uses simple scanline approach on path bounding boxes.
no external rasterizer needed.

---

### `plot.py`

stub for now. will later handle SVG → G-code → Moonraker or serial.

**current behavior:**
`plot(svg_path, config)`
copies SVG to output/current.svg.
logs "plotted" event.
returns immediately.

**future behavior (hardware phase):**
SVG → vpype for optimization → gcode plugin → HTTP POST to Moonraker
or pyserial write to printer port.

---

### `tui/ascii.py`

standalone utility. no dependencies beyond numpy.

`contours_to_ascii(contours, frame_shape, cols, rows) → str`
maps contour point coordinates into a cols×rows character grid.
character density: `█` > `▓` > `░` > ` `
weighted by number of contour points per cell.

`svg_to_ascii(svg_path, cols, rows) → str`
parses SVG path elements (only lines and polylines — no arcs).
rasterizes into same character grid format.
good enough for terminal preview; not pixel-accurate.

`graph_to_ascii(graph, cols, rows) → str`
shows primitive bounding boxes as labeled rectangles,
relation edges as ASCII lines between them.
labels: `T` triangle, `Q` quad, `P` pentagon, `H` hexagon, `L` line.

---

### `tui/panels.py`

four Textual Static widgets, updated independently via message bus.

**PerceptionPanel** — top left
shows: ascii contour preview, saliency score, contour count,
dominant primitive found.

**MemoryPanel** — top right
shows: two-column list (primitives | graphs).
each row: label, generation, observation count, status indicator.
sorted by generation descending (most elaborated first).
updates after every intellection phase.

**ElaborationPanel** — bottom left
shows: ascii SVG preview, current phase (COMPREHENSION / ACTION /
INTELLECTION / DORMANT), cycle counter, current form label,
generation number.

**LogPanel** — bottom right
shows: scrolling timestamped log lines, last 40 entries visible.
color-coded by phase: dim for dormant, blue for comprehension,
amber for action, green for intellection, red for errors.

**keybindings (in main.py):**
`R` — reset memory (confirm prompt before executing)
`P` — pause/resume after current cycle
`S` — step mode: advance one phase per keypress
`Q` — quit gracefully (finish current SVG write first)

---

### `main.py`

Textual App subclass. owns layout, keybindings, cycle task.

**startup:**
load config.toml.
create memory directory tree if not exists.
initialize camera (fail gracefully: log error, allow file input mode).
mount TUI panels.
if not paused: start cycle task.

**cycle task (async):**
runs in background via `asyncio.create_task`.
emits Textual messages at each phase boundary.
panels subscribe and update on receive.
never touches widgets directly — full decoupling.

**messages:**
`PhaseChanged(phase, data)` — panels update on this.
`CycleComplete(cycle_number)` — increments counter.
`MemoryUpdated(entries)` — memory panel refreshes.
`ErrorOccurred(message)` — log panel shows in red.

**file input mode:**
if no camera available, watches a directory for dropped JPEGs.
processes each file as a single cycle then waits for next.
useful for development and testing without hardware.

---

## build order

each step is independently testable before the next begins.

**step 1 — perception**
implement `perceive.py` and `tui/ascii.py`.
test with a static image: python -c "from perceive import *; print(contours_to_ascii(...))"
verify ASCII contour output looks reasonable.

**step 2 — recognition**
implement `recognize.py`.
feed it known shapes (draw a triangle in MS Paint, save as JPG).
verify hash stability: same shape → same hash across two runs.
verify circle rejection works.

**step 3 — memory**
implement `memory.py`.
test lookup/store/increment cycle independently.
verify filesystem layout matches spec.
verify log.jsonl appends correctly.

**step 4 — composition**
implement `compose.py`.
test with two known primitives at known positions.
verify all four relation types trigger correctly.
verify graph hash is order-independent.

**step 5 — elaboration**
implement `elaborate.py`.
test generation 0 → 1 → 2 on a known triangle.
verify SVG output is valid (open in browser or inkscape).
verify no arcs appear at any generation.
implement `svg_to_ascii` and verify preview renders.

**step 6 — TUI**
implement `tui/panels.py` and `main.py`.
wire message bus.
run full cycle on static image in step mode.
verify each panel updates at correct phase.

**step 7 — full loop**
enable live camera.
run autonomous loop.
verify memory accumulates correctly across cycles.
verify log is readable and append-only.

**step 8 — plot stub**
implement `plot.py` as stub.
verify SVG lands in output/ correctly.
(hardware integration deferred to next phase)

---

## dependencies

```
opencv-python
numpy
svgwrite
vpype
textual
tomllib        # stdlib in python 3.11+
pyserial       # stub import for now, hardware phase later
```

install:
```bash
pip install opencv-python numpy svgwrite vpype textual pyserial
```

libgl1 required for opencv headless on debian:
```bash
sudo apt install libgl1
```

---

## what this program is not

it is not a drawing tool. it is not an art generator.
it does not have goals. it does not optimize for anything.

it is a machine that looks at the world, finds what is closed and
non-circular in it, draws what it finds as simply as it can, and
remembers what it has already understood so it can go deeper next time.

the SVGs are not output. they are thoughts. they are erased.
only the memory persists.
