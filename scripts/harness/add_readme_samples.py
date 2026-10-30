#!/usr/bin/env python3
"""Adds four sessions to the harness fixture, one per OpenIntelligentUI README scene: a 3D airplane with pitch, roll
and yaw sliders, a plan comparison that becomes a chart, the bill splitter, and a coastal trip on a drawn map.
The pages carry everything themselves (canvas and SVG): the frame loads nothing from the web, so the README's live
map tiles and photos are drawn as shapes here.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_readme_samples.py scripts/harness/launch.sh
"""
import json
import os
from pathlib import Path

import build_fixture as bf
from add_generated_ui_samples import BILL, lines_for

PLANE = {
    "initialHeight": 420,
    "css": "canvas{display:block;width:100%;height:260px;border:1px solid var(--line);border-radius:8px;background:var(--bg)}.row{display:flex;gap:14px;align-items:center;margin-top:8px;flex-wrap:wrap}label{display:flex;gap:6px;align-items:center;color:var(--muted)}input[type=range]{width:120px}button{padding:5px 12px;border:0;border-radius:8px;background:var(--accent);color:#fff;cursor:pointer}.b2{background:var(--line);color:var(--fg)}",
    "html": "<canvas id='c' width='640' height='260'></canvas><div class='row'><label>Pitch <input id='pi' type='range' min='-60' max='60' value='15' oninput='draw()'></label><label>Roll <input id='ro' type='range' min='-90' max='90' value='20' oninput='draw()'></label><label>Yaw <input id='ya' type='range' min='-180' max='180' value='30' oninput='draw()'></label><button onclick='play()'>Play</button><button class='b2' onclick='reset()'>Reset</button></div>",
    "jsFunctions": "var M=[[0,0,60],[0,0,-50],[-6,0,-30],[6,0,-30],[0,10,-50],[0,0,-50],[-60,0,5],[60,0,5],[-60,0,-5],[60,0,-5],[-22,0,-48],[22,0,-48],[-22,0,-40],[22,0,-40]];var E=[[0,1],[6,7],[6,8],[7,9],[8,9],[10,11],[10,12],[11,13],[12,13],[4,5],[2,3],[0,2],[0,3],[2,1],[3,1]];function rot(p,a,b,c){var x=p[0],y=p[1],z=p[2],t;var ca=Math.cos(a),sa=Math.sin(a);t=y*ca-z*sa;z=y*sa+z*ca;y=t;var cb=Math.cos(b),sb=Math.sin(b);t=x*cb+z*sb;z=-x*sb+z*cb;x=t;var cc=Math.cos(c),sc=Math.sin(c);t=x*cc-y*sc;y=x*sc+y*cc;x=t;return[x,y,z]}function draw(){var cv=document.getElementById('c'),g=cv.getContext('2d'),w=cv.width,h=cv.height;g.clearRect(0,0,w,h);var s=getComputedStyle(document.documentElement);var fg=s.getPropertyValue('--fg')||'#000',ac=s.getPropertyValue('--accent')||'#c00';var a=+document.getElementById('pi').value*Math.PI/180,b=+document.getElementById('ya').value*Math.PI/180,c=+document.getElementById('ro').value*Math.PI/180;var P=M.map(function(p){var q=rot(p,a,b,c);var k=260/(260+q[2]);return[w/2+q[0]*k*1.6,h/2-q[1]*k*1.6]});g.lineWidth=2;g.strokeStyle=fg;E.forEach(function(e){g.beginPath();g.moveTo(P[e[0]][0],P[e[0]][1]);g.lineTo(P[e[1]][0],P[e[1]][1]);g.stroke()});g.fillStyle=ac;g.font='12px sans-serif';g.fillText('pitch '+document.getElementById('pi').value+'  roll '+document.getElementById('ro').value+'  yaw '+document.getElementById('ya').value,10,18)}var T=null;function play(){if(T){clearInterval(T);T=null;return}var n=0;T=setInterval(function(){n+=4;document.getElementById('ya').value=Math.round(180*Math.sin(n/30));document.getElementById('pi').value=Math.round(40*Math.sin(n/17));draw()},40)}function reset(){if(T){clearInterval(T);T=null}document.getElementById('pi').value=15;document.getElementById('ro').value=20;document.getElementById('ya').value=30;draw()}",
    "jsExpressions": "draw()",
}

CHART = {
    "initialHeight": 430,
    "css": "table{border-collapse:collapse;width:100%;margin-bottom:10px}th,td{padding:6px 10px;border-bottom:1px solid var(--line);text-align:right}th:first-child,td:first-child{text-align:left}button{padding:5px 12px;border:0;border-radius:8px;background:var(--accent);color:#fff;cursor:pointer}svg{display:none;width:100%;height:170px}",
    "html": "<table><tr><th>Plan</th><th>Price / mo</th><th>Seats</th><th>Per seat</th></tr><tr><td>Starter</td><td>$29</td><td>3</td><td>$9.67</td></tr><tr><td>Team</td><td>$99</td><td>15</td><td>$6.60</td></tr><tr><td>Business</td><td>$249</td><td>50</td><td>$4.98</td></tr></table><button id='b' onclick='show()'>Visualize cost per seat</button><svg id='s' viewBox='0 0 400 170'></svg><div class='note' style='color:var(--muted);font-size:12px'>Plan names and prices are illustrative.</div>",
    "jsFunctions": "function show(){var d=[['Starter',9.67],['Team',6.6],['Business',4.98]],s=document.getElementById('s'),h='';d.forEach(function(r,i){var bh=r[1]*10,x=40+i*120;h+='<rect x=\"'+x+'\" y=\"'+(140-bh)+'\" width=\"70\" height=\"'+bh+'\" rx=\"4\" fill=\"var(--accent)\"/><text x=\"'+(x+35)+'\" y=\"'+(132-bh)+'\" text-anchor=\"middle\" font-size=\"12\" fill=\"var(--fg)\">$'+r[1]+'</text><text x=\"'+(x+35)+'\" y=\"158\" text-anchor=\"middle\" font-size=\"12\" fill=\"var(--muted)\">'+r[0]+'</text>'});s.innerHTML=h;s.style.display='block';document.getElementById('b').style.display='none'}",
}

MAP = {
    "initialHeight": 380,
    "css": ".stops{display:flex;gap:8px;flex-wrap:wrap;margin-top:8px}.stop{padding:4px 10px;border:1px solid var(--line);border-radius:14px;background:none;color:var(--fg);cursor:pointer}.stop.on{background:var(--accent);color:#fff;border-color:var(--accent)}#info{margin-top:6px;color:var(--muted);font-size:13px;min-height:20px}.ctl{margin-top:8px}.ctl button{padding:5px 12px;border:0;border-radius:8px;background:var(--line);color:var(--fg);cursor:pointer;margin-right:6px}",
    "html": "<svg id='m' viewBox='0 0 400 230' style='width:100%;height:230px;border:1px solid var(--line);border-radius:8px'><path d='M0 0H400V230H0Z' fill='var(--bg)'/><path d='M0 0H230C210 40 250 70 215 105C190 135 215 160 185 190C170 205 150 215 140 230H0Z' fill='var(--line)' opacity='0.6'/><path id='route' fill='none' stroke='var(--accent)' stroke-width='2' stroke-dasharray='5 4'/><g id='pins'></g></svg><div class='stops' id='stops'></div><div id='info'>Pick a stop or press Replay.</div><div class='ctl'><button onclick='replay()'>Replay</button><button onclick='pause()'>Pause</button></div><div style='color:var(--muted);font-size:11px;margin-top:6px'>Connections are illustrative, not driving directions. Map drawn locally: the frame cannot load live tiles.</div>",
    "jsFunctions": "var S=[['Santa Cruz',230,40,'Boardwalk and surf breaks'],['Monterey',215,105,'Aquarium and Cannery Row'],['Big Sur',185,175,'Cliffs along Highway 1'],['San Simeon',150,215,'Hearst Castle']];var tm=null,idx=-1;function sel(i){idx=i;var h='';S.forEach(function(s,j){if(j<=i)h+='<g><circle cx=\"'+s[1]+'\" cy=\"'+s[2]+'\" r=\"10\" fill=\"var(--accent)\"/><text x=\"'+s[1]+'\" y=\"'+(s[2]+4)+'\" text-anchor=\"middle\" font-size=\"11\" fill=\"#fff\">'+(j+1)+'</text></g>'});document.getElementById('pins').innerHTML=h;var d='';S.forEach(function(s,j){if(j<=i)d+=(j?'L':'M')+s[1]+' '+s[2]});document.getElementById('route').setAttribute('d',d);document.querySelectorAll('.stop').forEach(function(b,j){b.className='stop'+(j===i?' on':'')});document.getElementById('info').textContent=i<0?'':S[i][0]+' - '+S[i][3]}function pause(){if(tm){clearInterval(tm);tm=null}}function replay(){pause();sel(-1);var n=0;tm=setInterval(function(){sel(n);n++;if(n>=S.length)pause()},900)}function build(){document.getElementById('stops').innerHTML=S.map(function(s,i){return '<button class=\"stop\" onclick=\"pause();sel('+i+')\">'+(i+1)+'. '+s[0]+'</button>'}).join('')}",
    "jsExpressions": "build();replay()",
}

SAMPLES = (
    ("00000000-0000-4000-8000-000000000501", "3D plane", PLANE, "Rotate the airplane with the sliders, or press Play.", 20),
    ("00000000-0000-4000-8000-000000000502", "Tables and charts", CHART, "Here is the comparison; press the button to see it as a chart.", 19),
    ("00000000-0000-4000-8000-000000000503", "Bill splitter", BILL, "Change the total, tip and group size.", 18),
    ("00000000-0000-4000-8000-000000000504", "Coastal trip", MAP, "Select a stop or replay the trip.", 17),
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
