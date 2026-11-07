(function(){
var A=__ARGS__;
var dpr=window.devicePixelRatio||1;
function at(){if(!A.coordinate)return null;var x=A.coordinate[0]/dpr,y=A.coordinate[1]/dpr;return {x:x,y:y,el:document.elementFromPoint(x,y)};}
function describe(el){if(!el)return 'nothing';var t=el.tagName.toLowerCase();var n=(el.getAttribute('aria-label')||el.innerText||el.value||'').replace(/\s+/g,' ').trim().slice(0,60);return '<'+t+(el.id?' id="'+el.id+'"':'')+'>'+(n?' "'+n+'"':'');}
function mouse(el,type,p,extra){el.dispatchEvent(new MouseEvent(type,Object.assign({bubbles:true,cancelable:true,view:window,clientX:p.x,clientY:p.y,button:0},extra||{})));}
if(A.action==='left_click'||A.action==='double_click'||A.action==='triple_click'||A.action==='right_click'||A.action==='hover'){
  var p=at();if(!p)return 'Error: pass coordinate [x, y] in screenshot pixels.';if(!p.el)return 'Error: nothing at '+A.coordinate.join(',')+'.';
  if(A.action==='hover'){mouse(p.el,'mouseover',p);mouse(p.el,'mousemove',p);return 'Hovered '+describe(p.el)+'.';}
  if(A.action==='right_click'){mouse(p.el,'contextmenu',p,{button:2});return 'Right-clicked '+describe(p.el)+'.';}
  var times=A.action==='double_click'?2:A.action==='triple_click'?3:1;
  for(var i=1;i<=times;i++){mouse(p.el,'mousedown',p,{detail:i});if(p.el.focus&&i===1)p.el.focus();mouse(p.el,'mouseup',p,{detail:i});if(typeof p.el.click==='function'&&i===1&&times===1){p.el.click();}else{mouse(p.el,'click',p,{detail:i});}}
  if(times===2)mouse(p.el,'dblclick',p,{detail:2});
  return (times===1?'Clicked ':times===2?'Double-clicked ':'Triple-clicked ')+describe(p.el)+'.';
}
if(A.action==='left_click_drag'){
  if(!A.start||!A.coordinate)return 'Error: pass start_coordinate and coordinate [x, y] in screenshot pixels.';
  var a={x:A.start[0]/dpr,y:A.start[1]/dpr},b={x:A.coordinate[0]/dpr,y:A.coordinate[1]/dpr};
  var src=document.elementFromPoint(a.x,a.y);if(!src)return 'Error: nothing at '+A.start.join(',')+'.';
  function ptr(el,type,p,buttons){try{el.dispatchEvent(new PointerEvent(type,{bubbles:true,cancelable:true,view:window,clientX:p.x,clientY:p.y,button:0,buttons:buttons,pointerId:1,pointerType:'mouse',isPrimary:true}));}catch(e){}}
  var drag=src.closest&&src.closest('[draggable="true"]');
  if(drag){
    var dt=new DataTransfer();
    function dnd(el,type,p){var e=new DragEvent(type,{bubbles:true,cancelable:true,view:window,clientX:p.x,clientY:p.y,dataTransfer:dt});el.dispatchEvent(e);return e;}
    if(dnd(drag,'dragstart',a).defaultPrevented)return 'The page refused to drag '+describe(drag)+'.';
    var over=document.elementFromPoint(b.x,b.y)||document.body;
    dnd(over,'dragenter',b);var ok=dnd(over,'dragover',b).defaultPrevented;
    if(ok)dnd(over,'drop',b);
    dnd(drag,'dragend',b);
    return ok?'Dragged '+describe(drag)+' onto '+describe(over)+'.':'Dragged '+describe(drag)+', but '+describe(over)+' does not take a drop.';
  }
  ptr(src,'pointerdown',a,1);mouse(src,'mousedown',a,{buttons:1});
  var steps=10,last=src;
  for(var s=1;s<=steps;s++){var q={x:a.x+(b.x-a.x)*s/steps,y:a.y+(b.y-a.y)*s/steps};var el3=document.elementFromPoint(q.x,q.y)||last;ptr(el3,'pointermove',q,1);mouse(el3,'mousemove',q,{buttons:1});last=el3;}
  ptr(last,'pointerup',b,0);mouse(last,'mouseup',b,{buttons:0});
  return 'Dragged from '+describe(src)+' to '+describe(last)+'.';
}
if(A.action==='type'){
  var el=document.activeElement;if(!el||el===document.body)return 'Error: nothing has the keyboard; click a field first.';
  var text=String(A.text||'');
  if('value' in el&&(el.tagName==='INPUT'||el.tagName==='TEXTAREA')){var proto=el.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;var d=Object.getOwnPropertyDescriptor(proto,'value');var start=el.selectionStart==null?el.value.length:el.selectionStart,end=el.selectionEnd==null?el.value.length:el.selectionEnd;var next=el.value.slice(0,start)+text+el.value.slice(end);d.set.call(el,next);try{el.setSelectionRange(start+text.length,start+text.length);}catch(e){}el.dispatchEvent(new InputEvent('input',{bubbles:true,data:text,inputType:'insertText'}));el.dispatchEvent(new Event('change',{bubbles:true}));}
  else if(el.isContentEditable){document.execCommand('insertText',false,text);}
  else return 'Error: '+describe(el)+' does not take text.';
  return 'Typed into '+describe(el)+'.';
}
if(A.action==='key'){
  var el2=document.activeElement||document.body;var keys=String(A.text||'').split(/\s+/).filter(Boolean);
  keys.forEach(function(k){var key=k==='Return'?'Enter':k==='space'?' ':k;var o={key:key,bubbles:true,cancelable:true};var down=el2.dispatchEvent(new KeyboardEvent('keydown',o));el2.dispatchEvent(new KeyboardEvent('keyup',o));
    if(down&&key==='Enter'&&el2.form&&el2.tagName==='INPUT'){if(el2.form.requestSubmit)el2.form.requestSubmit();else el2.form.submit();}});
  return 'Pressed '+keys.join(' ')+' in '+describe(el2)+'.';
}
if(A.action==='scroll_to'){var w=window.__forgeRefs&&window.__forgeRefs.byRef.get(A.ref);var t=w&&w.deref();if(!t)return 'Error: no element '+A.ref+'; call read_page first.';t.scrollIntoView({block:'center'});return 'Scrolled '+describe(t)+' into view.';}
return 'Error: unknown action';
})()
