(function(){
var A=__ARGS__;
var w=window.__forgeRefs&&window.__forgeRefs.byRef.get(A.ref);var el=w&&w.deref();
if(!el)return 'Error: no element '+A.ref+'; call read_page or find first.';
if(el.tagName!=='INPUT'||el.type!=='file')return 'Error: '+A.ref+' is not a file input (<'+el.tagName.toLowerCase()+(el.type?' type="'+el.type+'"':'')+'>).';
if(A.files.length>1&&!el.multiple)return 'Error: this file input takes one file.';
var dt=new DataTransfer();
A.files.forEach(function(f){var bin=atob(f.data);var bytes=new Uint8Array(bin.length);for(var i=0;i<bin.length;i++)bytes[i]=bin.charCodeAt(i);dt.items.add(new File([bytes],f.name,{type:f.type}));});
el.files=dt.files;
el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));
return 'Put '+A.files.map(function(f){return f.name;}).join(', ')+' into the file input.';
})()
