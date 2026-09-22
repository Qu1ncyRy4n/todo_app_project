"use strict";

const svg = document.querySelector("#graph");
const viewport = document.querySelector("#viewport");
const projectFilter = document.querySelector("#project-filter");
const tagFilter = document.querySelector("#tag-filter");
const details = document.querySelector("#details");
const summary = document.querySelector("#summary");
let allItems = [];
let projects = new Map();
let selectedId = null;
let zoom = 1;
let pan = { x: 0, y: 0 };
let dragStart = null;

document.querySelector("#file-input").addEventListener("change", async (event) => {
  const [file] = event.target.files;
  if (!file) return;
  try {
    const parsed = JSON.parse(await file.text());
    if (parsed?.format !== "todo-graph-v1" || !Array.isArray(parsed.items) || !parsed.items.every(isStoredItem) || !Array.isArray(parsed.projects)) {
      throw new Error("Expected a todo-graph-v1 JSON export.");
    }
    allItems = parsed.items;
    projects = new Map(parsed.projects.map((project) => [project.id, project.path]));
    selectedId = null;
    populateFilters();
    render();
  } catch (error) {
    summary.textContent = `Could not load export: ${error.message}`;
  }
});

projectFilter.addEventListener("change", render);
tagFilter.addEventListener("change", render);
document.querySelector("#zoom-in").addEventListener("click", () => changeZoom(1.2));
document.querySelector("#zoom-out").addEventListener("click", () => changeZoom(1 / 1.2));
document.querySelector("#zoom-reset").addEventListener("click", () => { zoom = 1; pan = { x: 0, y: 0 }; applyTransform(); });

svg.addEventListener("pointerdown", (event) => { dragStart = { x: event.clientX - pan.x, y: event.clientY - pan.y }; svg.setPointerCapture(event.pointerId); svg.classList.add("dragging"); });
svg.addEventListener("pointermove", (event) => { if (!dragStart) return; pan = { x: event.clientX - dragStart.x, y: event.clientY - dragStart.y }; applyTransform(); });
svg.addEventListener("pointerup", () => { dragStart = null; svg.classList.remove("dragging"); });
svg.addEventListener("wheel", (event) => { event.preventDefault(); changeZoom(event.deltaY < 0 ? 1.1 : 1 / 1.1); }, { passive: false });

function isStoredItem(value) {
  return value && typeof value === "object" && value.item && typeof value.item.id === "string" && Array.isArray(value.namespaces) && Array.isArray(value.blockers);
}

function projectOf(stored) { return stored.item.project_id ? (projects.get(stored.item.project_id) || "project/unresolved") : "global-inbox"; }

function tagsOf(stored) { return stored.namespaces.filter((tag) => !tag.startsWith("project/")); }

function populateFilters() {
  const projects = [...new Set(allItems.map(projectOf))].sort();
  const tags = [...new Set(allItems.flatMap(tagsOf))].sort();
  projectFilter.replaceChildren(new Option("All projects", "all"), ...projects.map((project) => new Option(project, project)));
  tagFilter.replaceChildren(...tags.map((tag) => new Option(tag, tag)));
}

function visibleItems() {
  const project = projectFilter.value;
  const tags = [...tagFilter.selectedOptions].map((option) => option.value);
  return allItems.filter((stored) => (project === "all" || projectOf(stored) === project) && tags.every((tag) => tagsOf(stored).includes(tag)));
}

function render() {
  const items = visibleItems();
  const visibleIds = new Set(items.map((stored) => stored.item.id));
  const positions = layout(items);
  viewport.replaceChildren();
  const edges = document.createElementNS(svg.namespaceURI, "g");
  const nodes = document.createElementNS(svg.namespaceURI, "g");
  for (const stored of items) {
    const source = positions.get(stored.item.id);
    if (stored.item.parent_id && visibleIds.has(stored.item.parent_id)) addEdge(edges, positions.get(stored.item.parent_id), source, "hierarchy", false);
    for (const blocker of stored.blockers) if (visibleIds.has(blocker)) addEdge(edges, positions.get(blocker), source, "dependency", true);
  }
  for (const [project, y] of laneStarts(items)) addLabel(nodes, project, y);
  for (const stored of items) addNode(nodes, stored, positions.get(stored.item.id));
  viewport.append(edges, nodes);
  summary.textContent = `${items.length} of ${allItems.length} items visible`;
  applyTransform();
  if (selectedId && visibleIds.has(selectedId)) setRelated(selectedId); else showDetails(null);
}

function laneStarts(items) {
  const result = new Map();
  let y = 60;
  for (const project of [...new Set(items.map(projectOf))].sort()) { result.set(project, y); y += 240; }
  return result;
}

function layout(items) {
  const positions = new Map();
  const lanes = laneStarts(items);
  for (const project of lanes.keys()) {
    const group = items.filter((stored) => projectOf(stored) === project).sort((a, b) => a.item.title.localeCompare(b.item.title));
    group.forEach((stored, index) => {
      const depth = ancestryDepth(stored, group);
      positions.set(stored.item.id, { id: stored.item.id, x: 72 + (index % 5) * 218, y: lanes.get(project) + 38 + depth * 72 + Math.floor(index / 5) * 108 });
    });
  }
  return positions;
}

function ancestryDepth(stored, group) {
  const ids = new Set();
  let parent = stored.item.parent_id;
  let depth = 0;
  while (parent && !ids.has(parent) && depth < 4) { ids.add(parent); parent = group.find((candidate) => candidate.item.id === parent)?.item.parent_id; depth += 1; }
  return depth;
}

function addLabel(group, project, y) {
  const label = document.createElementNS(svg.namespaceURI, "text");
  label.classList.add("lane-label"); label.setAttribute("x", "32"); label.setAttribute("y", String(y)); label.textContent = project.replace("project/", ""); group.append(label);
}

function addEdge(group, from, to, type, arrow) {
  if (!from || !to) return;
  const path = document.createElementNS(svg.namespaceURI, "path");
  path.classList.add("edge", type); path.dataset.from = from.id || ""; path.dataset.to = to.id || "";
  path.setAttribute("d", `M ${from.x + 168} ${from.y + 20} C ${from.x + 194} ${from.y + 20}, ${to.x - 26} ${to.y + 20}, ${to.x} ${to.y + 20}`);
  if (arrow) path.setAttribute("marker-end", "url(#arrow)");
  group.append(path);
}

function addNode(group, stored, position) {
  const item = stored.item;
  const node = document.createElementNS(svg.namespaceURI, "g");
  node.classList.add("node", item.state, `priority-${item.priority}`); node.dataset.id = item.id;
  node.setAttribute("transform", `translate(${position.x} ${position.y})`);
  const rect = document.createElementNS(svg.namespaceURI, "rect"); rect.setAttribute("width", "168"); rect.setAttribute("height", "48"); rect.setAttribute("rx", "7");
  const title = document.createElementNS(svg.namespaceURI, "text"); title.setAttribute("x", "10"); title.setAttribute("y", "20"); title.textContent = truncate(item.title, 23);
  const meta = document.createElementNS(svg.namespaceURI, "text"); meta.classList.add("meta"); meta.setAttribute("x", "10"); meta.setAttribute("y", "37"); meta.textContent = `${item.state} / ${item.priority}`;
  node.append(rect, title, meta);
  node.addEventListener("pointerenter", () => setRelated(item.id));
  node.addEventListener("pointerleave", () => { if (selectedId) setRelated(selectedId); else setRelated(null); });
  node.addEventListener("click", (event) => { event.stopPropagation(); selectedId = selectedId === item.id ? null : item.id; setRelated(selectedId); });
  group.append(node);
}

function setRelated(id) {
  const related = new Set(id ? [id] : []);
  const item = allItems.find((stored) => stored.item.id === id);
  if (item) {
    if (item.item.parent_id) related.add(item.item.parent_id);
    item.blockers.forEach((blocker) => related.add(blocker));
    allItems.filter((candidate) => candidate.item.parent_id === id || candidate.blockers.includes(id)).forEach((candidate) => related.add(candidate.item.id));
  }
  document.querySelectorAll(".node").forEach((node) => node.classList.toggle("related", related.has(node.dataset.id) && node.dataset.id !== id));
  document.querySelectorAll(".node").forEach((node) => node.classList.toggle("selected", node.dataset.id === id));
  document.querySelectorAll(".edge").forEach((edge) => edge.classList.toggle("related", related.has(edge.dataset.from) && related.has(edge.dataset.to)));
  showDetails(item);
}

function showDetails(stored) {
  details.replaceChildren();
  const heading = document.createElement("h2"); heading.textContent = "Selection"; details.append(heading);
  if (!stored) { const text = document.createElement("p"); text.className = "hint"; text.textContent = "Hover a node to trace its local relationships. Click to keep it selected."; details.append(text); return; }
  const title = document.createElement("strong"); title.textContent = stored.item.title;
  const facts = document.createElement("p"); facts.className = "hint"; facts.textContent = `${stored.item.state} / ${stored.item.priority}\n${stored.namespaces.join("\n") || "No tags"}\n${stored.blockers.length} blocker(s)`;
  details.append(title, facts);
}

function truncate(value, length) { return value.length > length ? `${value.slice(0, length - 1)}...` : value; }
function changeZoom(factor) { zoom = Math.max(0.45, Math.min(2.6, zoom * factor)); applyTransform(); }
function applyTransform() { viewport.setAttribute("transform", `translate(${pan.x} ${pan.y}) scale(${zoom})`); }
