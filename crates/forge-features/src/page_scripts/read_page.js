(function(){
var ARGS=__ARGS__;
var state=window.__forgeRefs||(function(){var s={next:1,byRef:new Map(),byEl:new WeakMap()};Object.defineProperty(window,'__forgeRefs',{value:s,enumerable:false});return s;})();
function refOf(el){var r=state.byEl.get(el);if(!r){r='ref_'+(state.next++);state.byEl.set(el,r);state.byRef.set(r,new WeakRef(el));}return r;}
var ROLES={A:'link',BUTTON:'button',SELECT:'combobox',TEXTAREA:'textbox',H1:'heading',H2:'heading',H3:'heading',H4:'heading',H5:'heading',H6:'heading',IMG:'img',UL:'list',OL:'list',LI:'listitem',NAV:'navigation',MAIN:'main',FORM:'form',HEADER:'banner',FOOTER:'contentinfo',TABLE:'table',TR:'row',TD:'cell',TH:'columnheader',LABEL:'label',SUMMARY:'button',DIALOG:'dialog',P:'paragraph',SECTION:'region',ARTICLE:'article',ASIDE:'complementary'};
function role(el){var r=el.getAttribute('role');if(r)return r;if(el.tagName==='INPUT'){var t=(el.type||'text').toLowerCase();return t==='checkbox'?'checkbox':t==='radio'?'radio':t==='submit'||t==='button'||t==='reset'?'button':t==='range'?'slider':'textbox';}if(el.tagName==='A'&&!el.hasAttribute('href'))return 'generic';return ROLES[el.tagName]||'generic';}
function interactive(el){var t=el.tagName;return (t==='A'&&el.hasAttribute('href'))||t==='BUTTON'||t==='INPUT'||t==='SELECT'||t==='TEXTAREA'||t==='SUMMARY'||el.hasAttribute('onclick')||['button','link','checkbox','radio','tab','menuitem','switch','textbox','combobox'].indexOf(el.getAttribute('role'))>=0||(el.hasAttribute('tabindex')&&el.getAttribute('tabindex')!=='-1')||el.isContentEditable;}
function name(el){var n=el.getAttribute('aria-label')||el.getAttribute('alt')||el.getAttribute('title')||'';if(!n&&el.id){var l=document.querySelector('label[for="'+CSS.escape(el.id)+'"]');if(l)n=l.innerText;}if(!n&&(el.tagName==='INPUT'||el.tagName==='SELECT'||el.tagName==='TEXTAREA')){var wrap=el.closest('label');if(wrap)n=wrap.innerText;}if(!n&&(el.tagName==='INPUT'||el.tagName==='TEXTAREA'))n=el.getAttribute('placeholder')||'';if(!n&&el.tagName!=='INPUT'&&el.tagName!=='SELECT'&&el.tagName!=='TEXTAREA'){var direct='';for(var c=el.firstChild;c;c=c.nextSibling){if(c.nodeType===3)direct+=c.textContent;}n=direct.trim()||(interactive(el)||/^H[1-6]$/.test(el.tagName)?(el.innerText||''):'');}return n.replace(/\s+/g,' ').trim().slice(0,120);}
function extra(el){var parts=[];if(el.tagName==='A'&&el.getAttribute('href'))parts.push('href="'+el.getAttribute('href')+'"');if(el.tagName==='INPUT'||el.tagName==='TEXTAREA'||el.tagName==='SELECT'){if(el.type==='checkbox'||el.type==='radio')parts.push(el.checked?'checked':'unchecked');else if(el.value)parts.push('value="'+String(el.value).slice(0,80)+'"');if(el.type&&el.tagName==='INPUT')parts.push('type="'+el.type+'"');}if(el.disabled)parts.push('disabled');return parts.length?' '+parts.join(' '):'';}
var SKIP={SCRIPT:1,STYLE:1,NOSCRIPT:1,TEMPLATE:1,META:1,LINK:1,HEAD:1};
var lines=[];
function walk(el,depth){if(depth>ARGS.depth||SKIP[el.tagName])return;var r=role(el),n=name(el);var show=ARGS.interactive?interactive(el):(r!=='generic'||n);var next=depth;if(show){lines.push('  '.repeat(depth)+r+(n?' "'+n+'"':'')+' ['+refOf(el)+']'+extra(el));next=depth+1;}for(var c=el.firstElementChild;c;c=c.nextElementSibling)walk(c,next);}
var root=document.body;if(ARGS.ref){var w=state.byRef.get(ARGS.ref);root=w&&w.deref();if(!root)return 'Error: no element '+ARGS.ref+' on this page; call read_page again.';}
if(!root)return 'The page has no body yet.';
walk(root,0);
var out=lines.join('\n');
if(out.length>ARGS.max){var cut=out.slice(0,ARGS.max);cut=cut.slice(0,cut.lastIndexOf('\n'));out=cut+'\n(cut at '+ARGS.max+' of '+out.length+' characters: pass a larger max_chars, or depth/ref_id)';}
return 'Title: '+document.title+'\nURL: '+location.href+'\n\n'+(out||'(no elements)');
})()
