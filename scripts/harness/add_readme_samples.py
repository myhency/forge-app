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
    "initialHeight": 520,
    "css": """#stage{position:relative;height:340px;border:1px solid var(--line);border-radius:10px;overflow:hidden;background:radial-gradient(ellipse at 50% 30%,color-mix(in srgb,var(--accent) 10%,var(--bg)),var(--bg) 70%);cursor:grab}#stage:active{cursor:grabbing}#stage canvas{display:block;width:100%;height:100%}.hint{position:absolute;left:10px;bottom:8px;font-size:11px;color:var(--muted);pointer-events:none}.panel{display:grid;grid-template-columns:repeat(3,1fr);gap:10px 18px;margin-top:12px}.ctl label{display:flex;justify-content:space-between;font-size:12px;color:var(--muted);margin-bottom:2px}.ctl b{color:var(--fg);font-variant-numeric:tabular-nums}.ctl input{width:100%;accent-color:var(--accent)}.actions{display:flex;gap:8px;margin-top:12px}.actions button{padding:6px 14px;border:0;border-radius:8px;cursor:pointer;font-weight:500}.p{background:var(--accent);color:#fff}.s{background:var(--line);color:var(--fg)}.legend{display:flex;gap:14px;font-size:12px;color:var(--muted);margin-top:10px}.legend i{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:5px}""",
    "html": """<script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script><div id="stage"><div class="hint">Drag to orbit · scroll to zoom</div></div><div class="panel"><div class="ctl"><label>Pitch <b id="vp">12°</b></label><input id="pi" type="range" min="-45" max="45" value="12"></div><div class="ctl"><label>Roll <b id="vr">-18°</b></label><input id="ro" type="range" min="-80" max="80" value="-18"></div><div class="ctl"><label>Yaw <b id="vy">25°</b></label><input id="ya" type="range" min="-180" max="180" value="25"></div></div><div class="actions"><button class="p" id="play">Play demonstration</button><button class="s" id="reset">Reset</button></div><div class="legend"><span><i style="background:#e5484d"></i>Pitch: nose up/down</span><span><i style="background:#30a46c"></i>Roll: bank left/right</span><span><i style="background:#3e63dd"></i>Yaw: turn left/right</span></div>""",
    "jsFunctions": """
var D=Math.PI/180,scene,cam,rend,plane,yawArrow,theta=0.9,phi=1.15,dist=15.5,playing=false,t0=0;
function mat(c,m,r){return new THREE.MeshStandardMaterial({color:c,metalness:m==null?0.2:m,roughness:r==null?0.45:r})}
function wing(pts,depth,color,flip){var sh=new THREE.Shape();pts.forEach(function(q,i){i?sh.lineTo(q[0],q[1]):sh.moveTo(q[0],q[1])});var g=new THREE.ExtrudeGeometry(sh,{depth:depth,bevelEnabled:true,bevelThickness:0.02,bevelSize:0.03,bevelSegments:2});var m=new THREE.Mesh(g,mat(color,0.25,0.4));m.rotation.x=flip?-Math.PI/2:Math.PI/2;return m}
function sprite(txt,col){var c=document.createElement('canvas');c.width=256;c.height=96;var g=c.getContext('2d');g.fillStyle=col;g.font='bold 54px sans-serif';g.textAlign='center';g.textBaseline='middle';g.fillText(txt,128,48);var sp=new THREE.Sprite(new THREE.SpriteMaterial({map:new THREE.CanvasTexture(c),transparent:true,depthTest:false}));sp.scale.set(2.4,0.9,1);return sp}
function build(){var G=new THREE.Group(),body=mat(0xf4f5f7,0.15,0.35),dark=mat(0x2b2f36,0.4,0.5),acc=mat(0xc4552d,0.2,0.4);
 var fus=new THREE.Mesh(new THREE.CylinderGeometry(0.55,0.55,5.2,40),body);fus.rotation.z=Math.PI/2;G.add(fus);
 var nose=new THREE.Mesh(new THREE.SphereGeometry(0.55,40,24,0,Math.PI*2,0,Math.PI/2),body);nose.rotation.z=-Math.PI/2;nose.position.x=2.6;nose.scale.set(1,1.9,1);G.add(nose);
 var tail=new THREE.Mesh(new THREE.ConeGeometry(0.55,2.2,40),body);tail.rotation.z=Math.PI/2;tail.position.x=-3.7;G.add(tail);
 var cock=new THREE.Mesh(new THREE.SphereGeometry(0.4,24,16),mat(0x1d3557,0.6,0.15));cock.scale.set(1.4,0.8,1);cock.position.set(2.2,0.32,0);G.add(cock);
 var band=new THREE.Mesh(new THREE.CylinderGeometry(0.56,0.56,0.28,40),acc);band.rotation.z=Math.PI/2;band.position.x=0.2;G.add(band);
 for(var i=0;i<9;i++){[-1,1].forEach(function(sd){var w=new THREE.Mesh(new THREE.BoxGeometry(0.2,0.16,0.04),dark);w.position.set(1.6-i*0.36,0.16,sd*0.545);G.add(w)})}
 var W=[[1.0,0],[-0.5,0],[-1.7,4.6],[-1.05,4.6]];var wr=wing(W,0.1,0xdfe3ea,false);wr.position.y=-0.2;G.add(wr);
 var wl=wing(W,0.1,0xdfe3ea,true);wl.position.y=-0.1;G.add(wl);
 [-1,1].forEach(function(sd){var tip=new THREE.Mesh(new THREE.BoxGeometry(0.5,0.7,0.06),acc);tip.position.set(-1.3,0.12,sd*4.6);tip.rotation.x=sd*0.12;G.add(tip);
  var e=new THREE.Mesh(new THREE.CylinderGeometry(0.36,0.4,1.5,32),mat(0xc9ced6,0.5,0.3));e.rotation.z=Math.PI/2;e.position.set(0.3,-0.65,sd*1.7);G.add(e);
  var inlet=new THREE.Mesh(new THREE.CylinderGeometry(0.3,0.3,0.05,32),dark);inlet.rotation.z=Math.PI/2;inlet.position.set(1.06,-0.65,sd*1.7);G.add(inlet)});
 var F=[[-2.6,0.3],[-3.9,0.3],[-4.4,2.1],[-3.7,2.1]];var fin=new THREE.Mesh(new THREE.ExtrudeGeometry((function(){var s=new THREE.Shape();F.forEach(function(q,i){i?s.lineTo(q[0],q[1]):s.moveTo(q[0],q[1])});return s})(),{depth:0.1,bevelEnabled:false}),acc);fin.position.z=-0.05;G.add(fin);
 var H=[[-3.3,0],[-4.1,0],[-4.6,1.9],[-4.1,1.9]];var hr=wing(H,0.07,0xdfe3ea,false);hr.position.y=0.1;G.add(hr);var hl=wing(H,0.07,0xdfe3ea,true);hl.position.y=0.1;G.add(hl);
 return G}
function axes(){var g=new THREE.Group();function ar(d,c,l,lab,p){var a=new THREE.ArrowHelper(d.normalize(),new THREE.Vector3(0,0,0),l,c,0.5,0.28);g.add(a);var s=sprite(lab,'#'+c.toString(16).padStart(6,'0'));s.position.copy(p);g.add(s)}
 ar(new THREE.Vector3(0,0,1),0xe5484d,5.2,'Pitch',new THREE.Vector3(0,0.3,6.2));ar(new THREE.Vector3(1,0,0),0x30a46c,5.6,'Roll',new THREE.Vector3(6.7,0.3,0));ar(new THREE.Vector3(0,1,0),0x3e63dd,3.4,'Yaw',new THREE.Vector3(0,4.4,0));return g}
function pose(){var p=+document.getElementById('pi').value,r=+document.getElementById('ro').value,y=+document.getElementById('ya').value;document.getElementById('vp').textContent=p+'°';document.getElementById('vr').textContent=r+'°';document.getElementById('vy').textContent=y+'°';plane.rotation.set(r*D,y*D,p*D,'YZX')}
function place(){cam.position.set(dist*Math.sin(phi)*Math.cos(theta),dist*Math.cos(phi),dist*Math.sin(phi)*Math.sin(theta));cam.lookAt(0,0,0)}
function frame(t){requestAnimationFrame(frame);if(playing){var s=(t-t0)/1000;document.getElementById('pi').value=Math.round(30*Math.sin(s*1.3));document.getElementById('ro').value=Math.round(60*Math.sin(s*0.9));document.getElementById('ya').value=Math.round(150*Math.sin(s*0.5));pose()}rend.render(scene,cam)}
function init(){var st=document.getElementById('stage'),w=st.clientWidth,h=st.clientHeight;scene=new THREE.Scene();cam=new THREE.PerspectiveCamera(38,w/h,0.1,100);rend=new THREE.WebGLRenderer({antialias:true,alpha:true});rend.setPixelRatio(Math.min(2,window.devicePixelRatio||1));rend.setSize(w,h);st.appendChild(rend.domElement);
 scene.add(new THREE.HemisphereLight(0xffffff,0x8a93a6,0.85));var dl=new THREE.DirectionalLight(0xffffff,0.9);dl.position.set(6,10,7);scene.add(dl);var dl2=new THREE.DirectionalLight(0xbcd0ff,0.35);dl2.position.set(-8,3,-6);scene.add(dl2);
 var grid=new THREE.GridHelper(20,20,0x9aa3b2,0xc9ced6);grid.position.y=-2.4;grid.material.transparent=true;grid.material.opacity=0.45;scene.add(grid);
 plane=build();scene.add(plane);scene.add(axes());pose();place();
 var drag=null;st.addEventListener('pointerdown',function(e){drag=[e.clientX,e.clientY];st.setPointerCapture(e.pointerId)});st.addEventListener('pointerup',function(){drag=null});st.addEventListener('pointermove',function(e){if(!drag)return;theta-=(e.clientX-drag[0])*0.008;phi=Math.max(0.25,Math.min(2.6,phi-(e.clientY-drag[1])*0.008));drag=[e.clientX,e.clientY];place()});st.addEventListener('wheel',function(e){e.preventDefault();dist=Math.max(6,Math.min(20,dist+e.deltaY*0.01));place()},{passive:false});
 ['pi','ro','ya'].forEach(function(id){document.getElementById(id).addEventListener('input',function(){playing=false;document.getElementById('play').textContent='Play demonstration';pose()})});
 document.getElementById('play').addEventListener('click',function(){playing=!playing;t0=performance.now();this.textContent=playing?'Pause':'Play demonstration'});
 document.getElementById('reset').addEventListener('click',function(){playing=false;document.getElementById('play').textContent='Play demonstration';document.getElementById('pi').value=12;document.getElementById('ro').value=-18;document.getElementById('ya').value=25;theta=0.9;phi=1.15;dist=15.5;pose();place()});
 requestAnimationFrame(frame)}
""",
    "jsExpressions": "init()",
}

CHART = {
    "initialHeight": 430,
    "css": "table{border-collapse:collapse;width:100%;margin-bottom:10px}th,td{padding:6px 10px;border-bottom:1px solid var(--line);text-align:right}th:first-child,td:first-child{text-align:left}button{padding:5px 12px;border:0;border-radius:8px;background:var(--accent);color:#fff;cursor:pointer}svg{display:none;width:100%;height:170px}",
    "html": "<table><tr><th>Plan</th><th>Price / mo</th><th>Seats</th><th>Per seat</th></tr><tr><td>Starter</td><td>$29</td><td>3</td><td>$9.67</td></tr><tr><td>Team</td><td>$99</td><td>15</td><td>$6.60</td></tr><tr><td>Business</td><td>$249</td><td>50</td><td>$4.98</td></tr></table><button id='b' onclick='show()'>Visualize cost per seat</button><svg id='s' viewBox='0 0 400 170'></svg><div class='note' style='color:var(--muted);font-size:12px'>Plan names and prices are illustrative.</div>",
    "jsFunctions": "function show(){var d=[['Starter',9.67],['Team',6.6],['Business',4.98]],s=document.getElementById('s'),h='';d.forEach(function(r,i){var bh=r[1]*10,x=40+i*120;h+='<rect x=\"'+x+'\" y=\"'+(140-bh)+'\" width=\"70\" height=\"'+bh+'\" rx=\"4\" fill=\"var(--accent)\"/><text x=\"'+(x+35)+'\" y=\"'+(132-bh)+'\" text-anchor=\"middle\" font-size=\"12\" fill=\"var(--fg)\">$'+r[1]+'</text><text x=\"'+(x+35)+'\" y=\"158\" text-anchor=\"middle\" font-size=\"12\" fill=\"var(--muted)\">'+r[0]+'</text>'});s.innerHTML=h;s.style.display='block';document.getElementById('b').style.display='none'}",
}

MAP = {
    "initialHeight": 600,
    "css": """#map{height:340px;border:1px solid var(--line);border-radius:10px}.pin{width:30px;height:30px;border-radius:50% 50% 50% 0;transform:rotate(-45deg);background:var(--accent);border:2px solid #fff;box-shadow:0 2px 6px rgba(0,0,0,.45);display:flex;align-items:center;justify-content:center;animation:drop .5s cubic-bezier(.3,1.6,.5,1)}.pin span{transform:rotate(45deg);color:#fff;font:700 13px sans-serif}.pin.on{background:#1f1e1c;transform:rotate(-45deg) scale(1.15)}@keyframes drop{from{opacity:0;margin-top:-40px}to{opacity:1}}.row{display:flex;gap:8px;flex-wrap:wrap;margin-top:10px}.stop{padding:5px 12px;border:1px solid var(--line);border-radius:16px;background:none;color:var(--fg);cursor:pointer;font-size:13px}.stop.on{background:var(--accent);border-color:var(--accent);color:#fff}.card{margin-top:10px;padding:12px 14px;border:1px solid var(--line);border-radius:10px;min-height:62px}.card b{display:block;margin-bottom:2px}.card span{color:var(--muted);font-size:13px}.act button{padding:5px 12px;border:0;border-radius:8px;background:var(--line);color:var(--fg);cursor:pointer;margin-right:6px}.note{color:var(--muted);font-size:11px;margin-top:8px}""",
    "html": """<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/leaflet.css"><script src="https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/leaflet.js"></script><div id="map"></div><div class="row" id="stops"></div><div class="card" id="card"><b>California Highway 1</b><span>Press Replay to drop the pins, or pick a stop.</span></div><div class="row act"><button id="rp">Replay</button><button id="ps">Pause</button></div><div class="note">Basemap: Esri World Imagery. Connections are illustrative, not verified driving directions.</div>""",
    "jsFunctions": """
var S=[{n:'Santa Cruz',ll:[36.9741,-122.0308],t:'Boardwalk, wharf and surf breaks at Steamer Lane.'},{n:'Monterey',ll:[36.6002,-121.8947],t:'Aquarium, Cannery Row and the Old Fisherman\\'s Wharf.'},{n:'Big Sur',ll:[36.3715,-121.9018],t:'Bixby Creek Bridge and cliffs above the Pacific.'},{n:'San Simeon',ll:[35.6852,-121.1668],t:'Hearst Castle and the elephant seal beaches.'}];
var map,markers=[],line,timer=null,cur=-1;
function icon(i,on){return L.divIcon({className:'',html:'<div class=\"pin'+(on?' on':'')+'\"><span>'+(i+1)+'</span></div>',iconSize:[30,30],iconAnchor:[15,30]})}
function show(i){cur=i;markers.forEach(function(m,j){m.setIcon(icon(j,j===i))});document.querySelectorAll('.stop').forEach(function(b,j){b.className='stop'+(j===i?' on':'')});if(i>=0){document.getElementById('card').innerHTML='<b>'+(i+1)+'. '+S[i].n+'</b><span>'+S[i].t+'</span>';map.flyTo(S[i].ll,i===0?11:10,{duration:1})}}
function stop(){if(timer){clearInterval(timer);timer=null}}
function replay(){stop();markers.forEach(function(m){map.removeLayer(m)});markers=[];line.setLatLngs([]);var n=0;map.fitBounds(S.map(function(s){return s.ll}),{padding:[30,30]});timer=setInterval(function(){if(n>=S.length){stop();return}var m=L.marker(S[n].ll,{icon:icon(n,false)}).addTo(map);(function(k){m.on('click',function(){stop();show(k)})})(n);markers.push(m);line.setLatLngs(S.slice(0,n+1).map(function(s){return s.ll}));show(n);n++},1500)}
function init(){map=L.map('map',{zoomControl:true,scrollWheelZoom:false}).setView([36.3,-121.6],8);L.tileLayer('https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}',{maxZoom:17,attribution:'Imagery: Esri, Maxar, Earthstar Geographics'}).addTo(map);line=L.polyline([],{color:'#c4552d',weight:3,dashArray:'8 8'}).addTo(map);
 document.getElementById('stops').innerHTML=S.map(function(s,i){return '<button class=\"stop\" data-i=\"'+i+'\">'+(i+1)+'. '+s.n+'</button>'}).join('');
 document.getElementById('stops').addEventListener('click',function(e){var b=e.target.closest('.stop');if(!b)return;stop();var i=+b.dataset.i;while(markers.length<=i){var k=markers.length;var m=L.marker(S[k].ll,{icon:icon(k,false)}).addTo(map);markers.push(m)}line.setLatLngs(S.slice(0,i+1).map(function(s){return s.ll}));show(i)});
 document.getElementById('rp').addEventListener('click',replay);document.getElementById('ps').addEventListener('click',stop);replay()}
""",
    "jsExpressions": "init()",
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
