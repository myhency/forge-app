#!/usr/bin/env python3
"""Adds the reviewed example pages (sidecar/src/ui_examples/*.json) and a few extras to the harness fixture, one
session each: the seven examples, a Korea and a world variant of the map, and an image test.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_readme_samples.py scripts/harness/launch.sh
"""
import json
import os
import re
from pathlib import Path

import build_fixture as bf
from add_generated_ui_samples import lines_for

EXAMPLES_DIR = Path(__file__).resolve().parent.parent.parent / "sidecar" / "src" / "ui_examples"


def load_examples():
    out = {}
    for path in sorted(EXAMPLES_DIR.glob("*.json")):
        if path.name == "kit.json":
            continue
        data = json.loads(path.read_text())
        out[data["name"]] = data
    return out


def map_variant(page, title, stops, center, zoom):
    """The map example with other stops: the same code, a different trip."""
    page = dict(page)
    js = page["jsFunctions"]
    js = re.sub(r"/\*STOPS\*/\[.*?\}\];", lambda _: "/*STOPS*/" + json.dumps(stops, ensure_ascii=False) + ";", js, count=1, flags=re.S)
    js = js.replace("center:[-121.6,36.3],zoom:6", f"center:{json.dumps(center)},zoom:{zoom}")
    page["jsFunctions"] = js
    page["html"] = page["html"].replace("California Highway 1", title).replace("Four stops from Santa Cruz to San Simeon.", f"{len(stops)} stops.")
    return page


IMAGES = {
    "initialHeight": 900,
    "css": "table{border-collapse:collapse;width:100%}td{padding:6px 8px;border-bottom:1px solid var(--line);vertical-align:middle;font-size:13px}td.im{width:120px}td.im>*{max-width:104px;max-height:70px;border:1px solid var(--line);border-radius:6px;background:var(--line);display:block}.ok{color:#30a46c;font-weight:600}.no{color:#e5484d;font-weight:600}.hint{color:var(--muted);font-size:12px}",
    "html": "<table id='t'></table><pre id='v' style='font-size:11px;color:var(--muted);white-space:pre-wrap'></pre><div class='hint' style='margin-top:8px'>Hosts that are not allowed must show 'blocked'; a missing Wikimedia file must also fail cleanly.</div>",
    "jsFunctions": """
var SVG='<svg xmlns="http://www.w3.org/2000/svg" width="104" height="70"><rect width="104" height="70" fill="#3e63dd"/><circle cx="52" cy="35" r="22" fill="#ffd166"/></svg>';
var P=[['Inline SVG in the page','inline'],['data: URI (SVG)','data:image/svg+xml;utf8,'+encodeURIComponent(SVG)],['Canvas drawing','canvas'],['jsDelivr npm: flag-icons (Korea)','https://cdn.jsdelivr.net/npm/flag-icons@7.2.3/flags/4x3/kr.svg'],['jsDelivr GitHub: twemoji (Korean flag)','https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/svg/1f1f0-1f1f7.svg'],['unpkg: Leaflet marker','https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png'],['cdnjs: Leaflet marker','https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/images/marker-icon.png'],['Wikimedia: Earth photo (upload host)','https://upload.wikimedia.org/wikipedia/commons/thumb/9/97/The_Earth_seen_from_Apollo_17.jpg/330px-The_Earth_seen_from_Apollo_17.jpg'],['Wikimedia: Special:FilePath redirect','https://commons.wikimedia.org/wiki/Special:FilePath/The_Earth_seen_from_Apollo_17.jpg?width=330'],['Wikimedia: missing file (page must handle it)','https://commons.wikimedia.org/wiki/Special:FilePath/This_file_does_not_exist_12345.jpg?width=320'],['Unsplash photo (not allowed, must stay blocked)','https://images.unsplash.com/photo-1506905925346-21bda4d32df4?w=200']];
function row(name){var tr=document.createElement('tr');tr.innerHTML='<td>'+name+'</td><td class="im"></td><td class="st">loading...</td>';document.getElementById('t').appendChild(tr);return tr}
function ok(tr,txt){tr.querySelector('.st').innerHTML='<span class="ok">'+txt+'</span>'}
function no(tr){tr.querySelector('.st').innerHTML='<span class="no">blocked</span>'}
document.addEventListener('securitypolicyviolation',function(e){var d=document.getElementById('v');d.textContent+=e.violatedDirective+' blocked '+e.blockedURI+'\\n'});
function run(){document.getElementById('v').textContent='CSP: '+document.querySelector('meta[http-equiv]').content.split('; ').filter(function(d){return d.indexOf('img-src')===0}).join('')+'\\n';P.forEach(function(p){var tr=row(p[0]),box=tr.querySelector('.im');
 if(p[1]==='inline'){box.innerHTML=SVG;ok(tr,'shown');return}
 if(p[1]==='canvas'){var c=document.createElement('canvas');c.width=104;c.height=70;var g=c.getContext('2d');var gr=g.createLinearGradient(0,0,104,70);gr.addColorStop(0,'#c4552d');gr.addColorStop(1,'#ffd166');g.fillStyle=gr;g.fillRect(0,0,104,70);g.fillStyle='#fff';g.font='bold 16px sans-serif';g.fillText('canvas',26,40);box.appendChild(c);ok(tr,'shown');return}
 var im=new Image();im.onload=function(){ok(tr,'loaded '+im.naturalWidth+'x'+im.naturalHeight)};im.onerror=function(){no(tr)};box.appendChild(im);im.src=p[1]})}
""",
    "jsExpressions": "run()",
}


EX = load_examples()
KOREA = map_variant(EX["map_trip"]["page"], "Korea road trip", [
    {"n": "서울", "ll": [126.978, 37.5665], "t": "경복궁, 북촌 한옥마을, 한강."},
    {"n": "경주", "ll": [129.2247, 35.8562], "t": "불국사와 신라 왕릉."},
    {"n": "부산", "ll": [129.0756, 35.1796], "t": "해운대, 감천문화마을, 자갈치시장."},
    {"n": "제주", "ll": [126.5312, 33.4996], "t": "한라산과 화산 해안, 성산일출봉."}], [127.8, 36.0], 5)
WORLD = map_variant(EX["map_trip"]["page"], "World tour", [
    {"n": "Reykjavik", "ll": [-21.9426, 64.1466], "t": "Geothermal pools and the northern lights."},
    {"n": "Cairo", "ll": [31.2357, 30.0444], "t": "The pyramids of Giza and the Nile."},
    {"n": "Nairobi", "ll": [36.8219, -1.2921], "t": "Gateway to the savanna and Nairobi National Park."},
    {"n": "서울", "ll": [126.978, 37.5665], "t": "Seoul: palaces and the Han River."},
    {"n": "Sydney", "ll": [151.2093, -33.8688], "t": "Harbour Bridge and the Opera House."},
    {"n": "Buenos Aires", "ll": [-58.3816, -34.6037], "t": "Tango, cafes and wide avenues."}], [20, 15], 1.3)

SAMPLES = tuple(
    (f"00000000-0000-4000-8000-0000000006{i:02d}", EX[name]["title"], EX[name]["page"], EX[name]["use_for"], 20 - i)
    for i, name in enumerate(["calculator", "comparison_chart", "map_trip", "explainer_3d", "tabbed_diagram", "checklist_planner", "quiz"])
) + (
    ("00000000-0000-4000-8000-000000000650", "Korea trip", KOREA, "서울, 경주, 부산, 제주를 OpenFreeMap으로 보여줍니다.", 12),
    ("00000000-0000-4000-8000-000000000651", "World tour", WORLD, "Six cities on six continents from one map.", 11),
    ("00000000-0000-4000-8000-000000000652", "Image test", IMAGES, "Which kinds of image the card can show.", 10),
)


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    project_dir = Path(claude_home) / "projects" / bf.escape_project_path(cwd)
    for session_id, title, page, reply, hour in SAMPLES:
        transcript = project_dir / f"{session_id}.jsonl"
        lines = lines_for(session_id, cwd, "Make a " + title.lower(), page, reply)
        lines.insert(1, {"type": "custom-title", "customTitle": title, "sessionId": session_id})
        transcript.write_text("".join(json.dumps(line) + "\n" for line in lines))
        mtime = bf._epoch(30, hour)
        os.utime(transcript, (mtime, mtime))


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
