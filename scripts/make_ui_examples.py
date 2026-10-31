#!/usr/bin/env python3
"""Writes the reviewed example pages for `ui_guide` (sidecar/src/ui_examples/*.json).

The examples are what Claude is shown as the bar for an interactive page, and what the evaluation measures new pages
against, so they use the Forge UI kit (sidecar/src/ui_examples/kit.md) and nothing else. Edit them here, then run:

    python3 scripts/make_ui_examples.py
"""
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "sidecar" / "src" / "ui_examples"

EXAMPLES = []


def example(name, title, use_for, page):
    EXAMPLES.append({"name": name, "title": title, "use_for": use_for, "page": page})


# --------------------------------------------------------------------------------------------------------------------
example("calculator", "Bill splitter", "A calculator or planner: a few inputs, a live result, a button that sends it back.", {
    "initialHeight": 320,
    "placeholderMessages": ["Setting up the calculator…"],
    "css": "",
    "html": """<div class="fx-stack">
  <div><h1 class="fx-title">Split the bill</h1><p class="fx-sub">Change the numbers; the share updates as you type.</p></div>
  <div class="fx-grid cols-3">
    <label class="fx-field"><span class="fx-label">Total ($)</span><input id="total" class="fx-input" type="number" min="0" step="0.01" value="120"></label>
    <label class="fx-field"><span class="fx-label">Tip <span id="tipv" class="fx-value">15%</span></span><input id="tip" class="fx-range" type="range" min="0" max="30" value="15"></label>
    <label class="fx-field"><span class="fx-label">People</span><input id="people" class="fx-input" type="number" min="1" step="1" value="4"></label>
  </div>
  <div class="fx-grid cols-3">
    <div class="fx-stat"><div class="fx-stat-value" id="each">$0.00</div><div class="fx-stat-label">Each pays</div></div>
    <div class="fx-stat"><div class="fx-stat-value" id="tipamt">$0.00</div><div class="fx-stat-label">Tip in total</div></div>
    <div class="fx-stat"><div class="fx-stat-value" id="grand">$0.00</div><div class="fx-stat-label">Grand total</div></div>
  </div>
  <div class="fx-row"><button class="fx-btn" id="send">Send this split</button><span class="fx-muted">Amounts are rounded to cents.</span></div>
</div>""",
    "jsFunctions": """function money(x){return '$'+(isFinite(x)?x:0).toFixed(2)}
function read(){var t=Math.max(0,parseFloat(document.getElementById('total').value)||0);var p=+document.getElementById('tip').value;var n=Math.max(1,Math.floor(+document.getElementById('people').value||1));return {t:t,p:p,n:n}}
function calc(){var v=read();var tip=v.t*v.p/100,grand=v.t+tip;document.getElementById('tipv').textContent=v.p+'%';document.getElementById('each').textContent=money(grand/v.n);document.getElementById('tipamt').textContent=money(tip);document.getElementById('grand').textContent=money(grand)}
function send(){var v=read();forge.send({label:'Bill split',values:{total:v.t,tip:v.p+'%',people:v.n,each:((v.t*(1+v.p/100))/v.n).toFixed(2)}})}""",
    "jsExpressions": """['total','tip','people'].forEach(function(id){document.getElementById(id).addEventListener('input',calc)});document.getElementById('send').addEventListener('click',send);calc()""",
})

# --------------------------------------------------------------------------------------------------------------------
example("comparison_chart", "Plans compared, then charted", "A comparison: a table of options and a chart of one metric the person can switch.", {
    "initialHeight": 420,
    "placeholderMessages": ["Comparing the options…"],
    "css": ".seg{display:inline-flex;border:1px solid var(--line);border-radius:9px;overflow:hidden}.seg button{border:0;background:none;padding:6px 12px;cursor:pointer;color:var(--muted);font-weight:600}.seg button.on{background:var(--accent);color:#fff}",
    "html": """<div class="fx-stack">
  <div><h1 class="fx-title">Which plan fits a team?</h1><p class="fx-sub">Prices are illustrative. Switch the metric to compare.</p></div>
  <table class="fx-table">
    <tr><th>Plan</th><th class="num">Price / month</th><th class="num">Seats</th><th class="num">Per seat</th></tr>
    <tr><td>Starter</td><td class="num">$29</td><td class="num">3</td><td class="num">$9.67</td></tr>
    <tr><td>Team</td><td class="num">$99</td><td class="num">15</td><td class="num">$6.60</td></tr>
    <tr><td>Business</td><td class="num">$249</td><td class="num">50</td><td class="num">$4.98</td></tr>
  </table>
  <div class="fx-row fx-spread"><span class="fx-label">Chart</span><div class="seg" id="seg"></div></div>
  <svg id="chart" class="fx-chart" viewBox="0 0 420 190" role="img" aria-label="Bar chart of the selected metric"></svg>
</div>""",
    "jsFunctions": """var PLANS=[{n:'Starter',price:29,seats:3},{n:'Team',price:99,seats:15},{n:'Business',price:249,seats:50}];
var METRICS={price:{label:'Price / month',f:function(p){return p.price},fmt:function(v){return '$'+v}},seats:{label:'Seats',f:function(p){return p.seats},fmt:function(v){return String(v)}},per:{label:'Per seat',f:function(p){return p.price/p.seats},fmt:function(v){return '$'+v.toFixed(2)}}};
function draw(key){var m=METRICS[key],vals=PLANS.map(m.f),max=Math.max.apply(null,vals)||1,h='';PLANS.forEach(function(p,i){var bh=Math.round(vals[i]/max*130),x=45+i*125;h+='<rect x="'+x+'" y="'+(160-bh)+'" width="70" height="'+bh+'" rx="6" fill="var(--c'+(i+1)+')"/><text x="'+(x+35)+'" y="'+(152-bh)+'" text-anchor="middle" font-size="13" font-weight="600" fill="var(--fg)">'+m.fmt(vals[i])+'</text><text x="'+(x+35)+'" y="180" text-anchor="middle" font-size="12" fill="var(--muted)">'+p.n+'</text>'});document.getElementById('chart').innerHTML=h;document.querySelectorAll('#seg button').forEach(function(b){b.className=b.dataset.k===key?'on':''})}
function build(){document.getElementById('seg').innerHTML=Object.keys(METRICS).map(function(k){return '<button data-k="'+k+'">'+METRICS[k].label+'</button>'}).join('');document.getElementById('seg').addEventListener('click',function(e){var b=e.target.closest('button');if(b)draw(b.dataset.k)})}""",
    "jsExpressions": "build();draw('per')",
})

# --------------------------------------------------------------------------------------------------------------------
STOPS_MARK = "/*STOPS*/"
example("map_trip", "A trip on a map", "Anything geographic anywhere in the world: stops on a real map with replay and details.", {
    "initialHeight": 560,
    "placeholderMessages": ["Drawing the map…"],
    "css": "#map{height:340px;border:1px solid var(--line);border-radius:12px}.pin{width:30px;height:30px;border-radius:50%;background:var(--accent);border:2px solid #fff;box-shadow:0 2px 6px rgba(0,0,0,.45);display:flex;align-items:center;justify-content:center;color:#fff;font:700 13px sans-serif;cursor:pointer;animation:drop .5s cubic-bezier(.3,1.6,.5,1)}.pin.on{background:#1f1e1c;transform:scale(1.2)}@keyframes drop{from{opacity:0;transform:translateY(-30px)}to{opacity:1}}.fx-btn.ghost.on{background:var(--accent);border-color:var(--accent);color:#fff}",
    "html": """<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/maplibre-gl/4.7.1/maplibre-gl.css"><script src="https://cdnjs.cloudflare.com/ajax/libs/maplibre-gl/4.7.1/maplibre-gl.js"></script>
<div class="fx-stack">
  <div><h1 class="fx-title">California Highway 1</h1><p class="fx-sub">Four stops from Santa Cruz to San Simeon.</p></div>
  <div id="map"></div>
  <div class="fx-row" id="stops"></div>
  <div class="fx-card" id="card"><div class="fx-label">Press Replay or pick a stop</div></div>
  <div class="fx-row"><button class="fx-btn" id="rp">Replay</button><button class="fx-btn ghost" id="ps">Pause</button><span class="fx-muted">Map: OpenFreeMap, data from OpenStreetMap. Routes are illustrative.</span></div>
</div>""",
    "jsFunctions": """var S=/*STOPS*/[{n:'Santa Cruz',ll:[-122.0308,36.9741],t:'Boardwalk, wharf and surf breaks at Steamer Lane.'},{n:'Monterey',ll:[-121.8947,36.6002],t:'Aquarium, Cannery Row and the old fishermans wharf.'},{n:'Big Sur',ll:[-121.9018,36.3715],t:'Bixby Creek Bridge and cliffs above the Pacific.'},{n:'San Simeon',ll:[-121.1668,35.6852],t:'Hearst Castle and the elephant seal beaches.'}];
var map,markers=[],timer=null;
function el(i){var o=document.createElement('div');var d=document.createElement('div');d.className='pin';d.textContent=i+1;o.appendChild(d);o.onclick=function(){stop();show(i)};return o}
function drawLine(n){var c=S.slice(0,n).map(function(s){return s.ll});var src=map.getSource('route');if(src)src.setData({type:'Feature',geometry:{type:'LineString',coordinates:c.length>1?c:[]}})}
function show(i){document.querySelectorAll('.stop').forEach(function(b,j){b.className='fx-btn ghost stop'+(j===i?' on':'')});markers.forEach(function(m,j){m.getElement().firstChild.className='pin'+(j===i?' on':'')});if(i>=0){document.getElementById('card').innerHTML='<div class="fx-title">'+(i+1)+'. '+S[i].n+'</div><p class="fx-sub">'+S[i].t+'</p>';map.flyTo({center:S[i].ll,zoom:10,duration:1400})}}
function stop(){if(timer){clearInterval(timer);timer=null}}
function fit(){var b=new maplibregl.LngLatBounds();S.forEach(function(s){b.extend(s.ll)});map.fitBounds(b,{padding:60,duration:900})}
function replay(){stop();markers.forEach(function(m){m.remove()});markers=[];drawLine(0);fit();var n=0;timer=setInterval(function(){if(n>=S.length){stop();return}var m=new maplibregl.Marker({element:el(n)}).setLngLat(S[n].ll).addTo(map);markers.push(m);drawLine(n+1);show(n);n++},1700)}
function init(){map=new maplibregl.Map({container:'map',style:'https://tiles.openfreemap.org/styles/liberty',center:[-121.6,36.3],zoom:6});map.addControl(new maplibregl.NavigationControl(),'top-right');
document.getElementById('stops').innerHTML=S.map(function(s,i){return '<button class="fx-btn ghost stop" data-i="'+i+'">'+(i+1)+'. '+s.n+'</button>'}).join('');
document.getElementById('stops').addEventListener('click',function(e){var b=e.target.closest('.stop');if(!b)return;stop();var i=+b.dataset.i;while(markers.length<=i){var k=markers.length;markers.push(new maplibregl.Marker({element:el(k)}).setLngLat(S[k].ll).addTo(map))}drawLine(i+1);show(i)});
document.getElementById('rp').addEventListener('click',replay);document.getElementById('ps').addEventListener('click',stop);
map.on('load',function(){map.addSource('route',{type:'geojson',data:{type:'Feature',geometry:{type:'LineString',coordinates:[]}}});map.addLayer({id:'route',type:'line',source:'route',paint:{'line-color':'#c4552d','line-width':3,'line-dasharray':[2,2]}});replay()})}""",
    "jsExpressions": "init()",
})

# --------------------------------------------------------------------------------------------------------------------
example("explainer_3d", "A 3D model to explore", "Something physical or spatial: a model built from geometry with sliders, drag to rotate, a demo.", {
    "initialHeight": 560,
    "placeholderMessages": ["Building the model…"],
    "css": "#stage{position:relative;height:340px;border:1px solid var(--line);border-radius:12px;overflow:hidden;background:radial-gradient(ellipse at 50% 30%,color-mix(in srgb,var(--accent) 10%,var(--bg)),var(--bg) 70%);cursor:grab;touch-action:none}#stage canvas{display:block;width:100%;height:100%}.hint{position:absolute;left:10px;bottom:8px;font-size:11px;color:var(--muted);pointer-events:none}",
    "html": """<script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
<div class="fx-stack">
  <div><h1 class="fx-title">Pitch, roll and yaw</h1><p class="fx-sub">The three ways an airplane turns around its centre.</p></div>
  <div id="stage"><div class="hint">Drag to orbit · scroll to zoom</div></div>
  <div class="fx-grid cols-3">
    <label class="fx-field"><span class="fx-label">Pitch (nose up/down) <span class="fx-value" id="vp">12°</span></span><input id="pi" class="fx-range" type="range" min="-45" max="45" value="12"></label>
    <label class="fx-field"><span class="fx-label">Roll (bank) <span class="fx-value" id="vr">-18°</span></span><input id="ro" class="fx-range" type="range" min="-80" max="80" value="-18"></label>
    <label class="fx-field"><span class="fx-label">Yaw (turn) <span class="fx-value" id="vy">25°</span></span><input id="ya" class="fx-range" type="range" min="-180" max="180" value="25"></label>
  </div>
  <div class="fx-row"><button class="fx-btn" id="play">Play demonstration</button><button class="fx-btn ghost" id="reset">Reset</button></div>
</div>""",
    "jsFunctions": """var D=Math.PI/180,scene,cam,rend,plane,theta=0.9,phi=1.15,dist=15.5,playing=false,t0=0;
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
var W=[[1.0,0],[-0.5,0],[-1.7,4.6],[-1.05,4.6]];var wr=wing(W,0.1,0xdfe3ea,false);wr.position.y=-0.2;G.add(wr);var wl=wing(W,0.1,0xdfe3ea,true);wl.position.y=-0.1;G.add(wl);
[-1,1].forEach(function(sd){var tip=new THREE.Mesh(new THREE.BoxGeometry(0.5,0.7,0.06),acc);tip.position.set(-1.3,0.12,sd*4.6);tip.rotation.x=sd*0.12;G.add(tip);var e=new THREE.Mesh(new THREE.CylinderGeometry(0.36,0.4,1.5,32),mat(0xc9ced6,0.5,0.3));e.rotation.z=Math.PI/2;e.position.set(0.3,-0.65,sd*1.7);G.add(e);var inlet=new THREE.Mesh(new THREE.CylinderGeometry(0.3,0.3,0.05,32),dark);inlet.rotation.z=Math.PI/2;inlet.position.set(1.06,-0.65,sd*1.7);G.add(inlet)});
var F=[[-2.6,0.3],[-3.9,0.3],[-4.4,2.1],[-3.7,2.1]];var fs=new THREE.Shape();F.forEach(function(q,i){i?fs.lineTo(q[0],q[1]):fs.moveTo(q[0],q[1])});var fin=new THREE.Mesh(new THREE.ExtrudeGeometry(fs,{depth:0.1,bevelEnabled:false}),acc);fin.position.z=-0.05;G.add(fin);
var H=[[-3.3,0],[-4.1,0],[-4.6,1.9],[-4.1,1.9]];var hr=wing(H,0.07,0xdfe3ea,false);hr.position.y=0.1;G.add(hr);var hl=wing(H,0.07,0xdfe3ea,true);hl.position.y=0.1;G.add(hl);return G}
function axes(){var g=new THREE.Group();function ar(d,c,l,lab,p){g.add(new THREE.ArrowHelper(d.normalize(),new THREE.Vector3(0,0,0),l,c,0.5,0.28));var s=sprite(lab,'#'+c.toString(16).padStart(6,'0'));s.position.copy(p);g.add(s)}
ar(new THREE.Vector3(0,0,1),0xe5484d,5.2,'Pitch',new THREE.Vector3(0,0.3,6.2));ar(new THREE.Vector3(1,0,0),0x30a46c,5.6,'Roll',new THREE.Vector3(6.7,0.3,0));ar(new THREE.Vector3(0,1,0),0x3e63dd,3.4,'Yaw',new THREE.Vector3(0,4.4,0));return g}
function pose(){var p=+document.getElementById('pi').value,r=+document.getElementById('ro').value,y=+document.getElementById('ya').value;document.getElementById('vp').textContent=p+'°';document.getElementById('vr').textContent=r+'°';document.getElementById('vy').textContent=y+'°';plane.rotation.set(r*D,y*D,p*D,'YZX')}
function place(){cam.position.set(dist*Math.sin(phi)*Math.cos(theta),dist*Math.cos(phi),dist*Math.sin(phi)*Math.sin(theta));cam.lookAt(0,0,0)}
function frame(t){requestAnimationFrame(frame);if(playing){var s=(t-t0)/1000;document.getElementById('pi').value=Math.round(30*Math.sin(s*1.3));document.getElementById('ro').value=Math.round(60*Math.sin(s*0.9));document.getElementById('ya').value=Math.round(150*Math.sin(s*0.5));pose()}rend.render(scene,cam)}
function init(){var st=document.getElementById('stage'),w=st.clientWidth,h=st.clientHeight;scene=new THREE.Scene();cam=new THREE.PerspectiveCamera(38,w/h,0.1,100);rend=new THREE.WebGLRenderer({antialias:true,alpha:true});rend.setPixelRatio(Math.min(2,window.devicePixelRatio||1));rend.setSize(w,h);st.appendChild(rend.domElement);
scene.add(new THREE.HemisphereLight(0xffffff,0x8a93a6,0.85));var dl=new THREE.DirectionalLight(0xffffff,0.9);dl.position.set(6,10,7);scene.add(dl);var dl2=new THREE.DirectionalLight(0xbcd0ff,0.35);dl2.position.set(-8,3,-6);scene.add(dl2);
var grid=new THREE.GridHelper(20,20,0x9aa3b2,0xc9ced6);grid.position.y=-2.4;grid.material.transparent=true;grid.material.opacity=0.45;scene.add(grid);
plane=build();scene.add(plane);scene.add(axes());pose();place();
var drag=null;st.addEventListener('pointerdown',function(e){drag=[e.clientX,e.clientY];st.setPointerCapture(e.pointerId)});st.addEventListener('pointerup',function(){drag=null});st.addEventListener('pointermove',function(e){if(!drag)return;theta-=(e.clientX-drag[0])*0.008;phi=Math.max(0.25,Math.min(2.6,phi-(e.clientY-drag[1])*0.008));drag=[e.clientX,e.clientY];place()});st.addEventListener('wheel',function(e){e.preventDefault();dist=Math.max(6,Math.min(24,dist+e.deltaY*0.01));place()},{passive:false});
['pi','ro','ya'].forEach(function(id){document.getElementById(id).addEventListener('input',function(){playing=false;document.getElementById('play').textContent='Play demonstration';pose()})});
document.getElementById('play').addEventListener('click',function(){playing=!playing;t0=performance.now();this.textContent=playing?'Pause':'Play demonstration'});
document.getElementById('reset').addEventListener('click',function(){playing=false;document.getElementById('play').textContent='Play demonstration';document.getElementById('pi').value=12;document.getElementById('ro').value=-18;document.getElementById('ya').value=25;theta=0.9;phi=1.15;dist=15.5;pose();place()});
requestAnimationFrame(frame)}""",
    "jsExpressions": "init()",
})

# --------------------------------------------------------------------------------------------------------------------
example("tabbed_diagram", "How a wing makes lift", "An explanation with several views of one thing: tabs, a diagram, a slider that changes it.", {
    "initialHeight": 460,
    "placeholderMessages": ["Drawing the diagram…"],
    "css": ".foil{transition:transform .25s ease;transform-origin:210px 100px}",
    "html": """<div class="fx-stack">
  <div><h1 class="fx-title">How a wing makes lift</h1><p class="fx-sub">Air is pushed down; the wing is pushed up.</p></div>
  <div class="fx-tabset">
    <div class="fx-tabs"><button class="fx-tab" data-tab="air">Airflow</button><button class="fx-tab" data-tab="pressure">Pressure</button><button class="fx-tab" data-tab="try">Try the angle</button></div>
    <div class="fx-panel" data-panel="air"><p>Air meeting the curved top of the wing is bent downward. By Newton's third law the wing is pushed the other way: that push is lift.</p></div>
    <div class="fx-panel" data-panel="pressure"><p>The air moving over the top is faster, and faster air has lower pressure. Higher pressure underneath and lower pressure on top add up to an upward force.</p></div>
    <div class="fx-panel" data-panel="try">
      <div class="fx-grid cols-2">
        <label class="fx-field"><span class="fx-label">Angle of attack <span class="fx-value" id="av">6°</span></span><input id="angle" class="fx-range" type="range" min="-5" max="22" value="6"></label>
        <div class="fx-stat"><div class="fx-stat-value" id="cl">0.66</div><div class="fx-stat-label" id="state">Lift coefficient</div></div>
      </div>
    </div>
  </div>
  <svg id="d" class="fx-chart" viewBox="0 0 420 200" role="img" aria-label="Airflow around a wing section">
    <g stroke="var(--c2)" stroke-width="1.6" fill="none" opacity="0.7"><path d="M10 40 C140 40 300 30 410 60"/><path d="M10 70 C140 66 300 56 410 80"/><path d="M10 130 C140 140 300 150 410 128"/><path d="M10 160 C140 166 300 172 410 152"/></g>
    <g class="foil" id="foil"><path d="M120 100 C150 70 270 70 310 100 C270 112 160 112 120 100 Z" fill="var(--c1)" opacity="0.9"/></g>
    <g id="arrow" stroke="var(--c3)" stroke-width="3"><line x1="210" y1="96" x2="210" y2="40"/><path d="M203 48 L210 36 L217 48" fill="none"/></g>
    <text x="222" y="34" font-size="12" fill="var(--c3)" font-weight="600">lift</text>
  </svg>
</div>""",
    "jsFunctions": """function setAngle(){var a=+document.getElementById('angle').value;var cl=a>15?Math.max(0.2,1.65-(a-15)*0.18):0.11*a+0.2;document.getElementById('av').textContent=a+'°';document.getElementById('cl').textContent=cl.toFixed(2);document.getElementById('state').textContent=a>15?'Stalled: the air no longer follows the wing':'Lift coefficient';document.getElementById('foil').style.transform='rotate('+(-a)+'deg)';var len=Math.max(8,Math.min(70,cl*42));var ar=document.getElementById('arrow');ar.innerHTML='<line x1="210" y1="96" x2="210" y2="'+(96-len)+'"/><path d="M203 '+(104-len)+' L210 '+(92-len)+' L217 '+(104-len)+'" fill="none"/>'}""",
    "jsExpressions": """document.getElementById('angle').addEventListener('input',setAngle);setAngle()""",
})

# --------------------------------------------------------------------------------------------------------------------
example("checklist_planner", "A plan with a checklist", "A plan someone follows: a quantity that scales the list, steps to tick off, progress, a button to send the result.", {
    "initialHeight": 560,
    "placeholderMessages": ["Planning the meal…"],
    "css": ".qty button{width:32px;height:32px;padding:0}",
    "html": """<div class="fx-stack">
  <div class="fx-row fx-spread">
    <div><h1 class="fx-title">Sunday roast</h1><p class="fx-sub">Lamb, potatoes and greens, timed backwards from serving.</p></div>
    <div class="fx-row qty"><button class="fx-btn ghost" id="minus" aria-label="Fewer">−</button><span class="fx-value" id="serv">4 people</span><button class="fx-btn ghost" id="plus" aria-label="More">+</button></div>
  </div>
  <div class="fx-grid cols-2">
    <div class="fx-card"><div class="fx-label">Shopping list</div><table class="fx-table" id="shop"></table></div>
    <div class="fx-card"><div class="fx-row fx-spread"><span class="fx-label">Timeline</span><span class="fx-muted" id="prog">0 of 5 done</span></div><div class="fx-bar"><span id="barfill" style="width:0%"></span></div><div id="steps"></div></div>
  </div>
  <div class="fx-row"><button class="fx-btn" id="send">Send the shopping list</button></div>
</div>""",
    "jsFunctions": """var ITEMS=[['Leg of lamb',0.5,'kg'],['Potatoes',0.3,'kg'],['Carrots',0.2,'kg'],['Green beans',0.15,'kg'],['Garlic',0.5,'bulb'],['Rosemary',0.5,'bunch']];
var STEPS=['Take the lamb out of the fridge (2 h before)','Heat the oven to 200°C and season the lamb','Roast the lamb; add the potatoes after 30 min','Steam the carrots and beans (15 min before)','Rest the lamb 20 min, then carve'];
var serv=4;
function q(x){var v=x*serv;return (Math.round(v*10)/10).toString().replace(/\\.0$/,'')}
function shop(){document.getElementById('serv').textContent=serv+(serv===1?' person':' people');document.getElementById('shop').innerHTML=ITEMS.map(function(i){return '<tr><td>'+i[0]+'</td><td class="num">'+q(i[1])+' '+i[2]+'</td></tr>'}).join('')}
function prog(){var all=document.querySelectorAll('#steps input'),done=document.querySelectorAll('#steps input:checked').length;document.getElementById('prog').textContent=done+' of '+all.length+' done';document.getElementById('barfill').style.width=(all.length?done/all.length*100:0)+'%'}
function steps(){document.getElementById('steps').innerHTML=STEPS.map(function(s){return '<label class="fx-check"><input type="checkbox"><span>'+s+'</span></label>'}).join('');document.getElementById('steps').addEventListener('change',prog);prog()}
function send(){forge.send({label:'Shopping list',values:{servings:serv,items:ITEMS.map(function(i){return i[0]+' '+q(i[1])+i[2]}).join(', ').slice(0,190)}})}""",
    "jsExpressions": """document.getElementById('minus').addEventListener('click',function(){serv=Math.max(1,serv-1);shop()});document.getElementById('plus').addEventListener('click',function(){serv=Math.min(20,serv+1);shop()});document.getElementById('send').addEventListener('click',send);shop();steps()""",
})

# --------------------------------------------------------------------------------------------------------------------
example("quiz", "A short quiz", "Test understanding: questions one at a time, instant feedback, a score, and the result sent back.", {
    "initialHeight": 380,
    "placeholderMessages": ["Writing the questions…"],
    "css": ".opt{display:block;width:100%;text-align:left;margin-top:8px}.opt.right{border-color:var(--c3);background:color-mix(in srgb,var(--c3) 16%,transparent)}.opt.wrong{border-color:#e5484d;background:color-mix(in srgb,#e5484d 14%,transparent)}",
    "html": """<div class="fx-stack">
  <div class="fx-row fx-spread"><div><h1 class="fx-title">Monty Hall quiz</h1><p class="fx-sub" id="count">Question 1 of 3</p></div><span class="fx-badge" id="score">0 correct</span></div>
  <div class="fx-bar"><span id="bar" style="width:0%"></span></div>
  <div class="fx-card" id="q"></div>
  <div class="fx-callout" id="why" hidden></div>
  <div class="fx-row"><button class="fx-btn" id="next" hidden>Next</button></div>
</div>""",
    "jsFunctions": """var Q=[{q:'You pick door 1 of 3. The host, who knows where the prize is, opens a goat door. What is the best move?',o:['Stay with door 1','Switch to the other closed door','It makes no difference'],a:1,w:'Switching wins 2 times out of 3: your first pick was right only 1 time in 3, and the host never reveals the prize.'},{q:'Which part of the rules makes switching better?',o:['The host opens a random door','The host always opens a goat door','There are three doors'],a:1,w:'Because the host never opens the prize door, his choice carries information.'},{q:'With 100 doors and 98 goat doors opened, what are the odds the other closed door has the prize?',o:['1 in 100','1 in 2','99 in 100'],a:2,w:'Your first pick stays 1 in 100, so the other door holds the remaining 99 in 100.'}];
var i=0,ok=0,locked=false;
function show(){locked=false;document.getElementById('count').textContent='Question '+(i+1)+' of '+Q.length;document.getElementById('bar').style.width=(i/Q.length*100)+'%';var c=Q[i];document.getElementById('q').innerHTML='<div class="fx-title">'+c.q+'</div>'+c.o.map(function(t,k){return '<button class="fx-btn ghost opt" data-k="'+k+'">'+t+'</button>'}).join('');document.getElementById('why').hidden=true;document.getElementById('next').hidden=true}
function pick(k){if(locked)return;locked=true;var c=Q[i];if(k===c.a)ok++;document.querySelectorAll('.opt').forEach(function(b,j){if(j===c.a)b.classList.add('right');else if(j===k)b.classList.add('wrong')});document.getElementById('score').textContent=ok+' correct';var w=document.getElementById('why');w.textContent=c.w;w.hidden=false;var n=document.getElementById('next');n.textContent=i===Q.length-1?'See result':'Next';n.hidden=false}
function next(){if(i<Q.length-1){i++;show();return}document.getElementById('bar').style.width='100%';document.getElementById('count').textContent='Done';document.getElementById('q').innerHTML='<div class="fx-stat-value">'+ok+' / '+Q.length+'</div><p class="fx-sub">'+(ok===Q.length?'Perfect.':'Read the explanations again and try once more.')+'</p><div class="fx-row"><button class="fx-btn" id="send">Send my score</button></div>';document.getElementById('why').hidden=true;document.getElementById('next').hidden=true;document.getElementById('send').addEventListener('click',function(){forge.send({label:'Monty Hall quiz',values:{score:ok,of:Q.length}})})}""",
    "jsExpressions": """document.getElementById('q').addEventListener('click',function(e){var b=e.target.closest('.opt');if(b)pick(+b.dataset.k)});document.getElementById('next').addEventListener('click',next);show()""",
})


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.json"):
        old.unlink()  # kit.json is rewritten below from kit.md
    for ex in EXAMPLES:
        (OUT / f"{ex['name']}.json").write_text(json.dumps(ex, ensure_ascii=False, indent=2) + "\n")
    # The packaged app copies only the compiled sidecar (dist), which carries JSON but not markdown.
    kit = (OUT / "kit.md").read_text()
    (OUT / "kit.json").write_text(json.dumps({"markdown": kit}, ensure_ascii=False, indent=2) + "\n")
    print(f"wrote {len(EXAMPLES)} examples and kit.json to {OUT}")


if __name__ == "__main__":
    main()
