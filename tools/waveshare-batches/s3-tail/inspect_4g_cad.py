"""Enumerate every native solid/open shell/face and source material, without transforms."""
from pathlib import Path
import argparse,importlib.util,json
from OCP.STEPCAFControl import STEPCAFControl_Reader
from OCP.TDocStd import TDocStd_Document
from OCP.TCollection import TCollection_ExtendedString
from OCP.TopExp import TopExp_Explorer
from OCP.TopAbs import TopAbs_SOLID,TopAbs_SHELL,TopAbs_FACE
from OCP.XCAFDoc import XCAFDoc_DocumentTool,XCAFDoc_ColorType
from OCP.Quantity import Quantity_Color
ap=argparse.ArgumentParser();ap.add_argument('source',type=Path);ap.add_argument('--output',type=Path,required=True);a=ap.parse_args();spec=importlib.util.spec_from_file_location('builder',Path(__file__).resolve().parents[3]/'tools/build-waveshare-models.py');b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
doc=TDocStd_Document(TCollection_ExtendedString('XmlXCAF'));r=STEPCAFControl_Reader();r.SetColorMode(True);r.SetNameMode(True);assert r.ReadFile(str(a.source)).value==1;assert r.Transfer(doc);shape=r.Reader().OneShape();ct=XCAFDoc_DocumentTool.ColorTool_s(doc.Main());items=[]
for kind,avoid in [(TopAbs_SOLID,None),(TopAbs_SHELL,TopAbs_SOLID),(TopAbs_FACE,TopAbs_SHELL)]:
 e=TopExp_Explorer(shape,kind)if avoid is None else TopExp_Explorer(shape,kind,avoid)
 while e.More():
  s=e.Current();bb=b.bounds(s);v={'index':len(items),'bounds':bb.tolist(),'size':(bb[1]-bb[0]).tolist(),'center':bb.mean(axis=0).tolist(),'topology':kind.name};entity=r.Reader().WS().TransferReader().EntityFromShapeResult(s,1)
  if entity is not None and hasattr(entity,'Name'):v['name']=entity.Name().ToCString()
  c=Quantity_Color()
  if ct.GetColor(s,XCAFDoc_ColorType.XCAFDoc_ColorSurf,c)or ct.GetColor(s,XCAFDoc_ColorType.XCAFDoc_ColorGen,c):v['color']=[c.Red(),c.Green(),c.Blue()]
  items.append(v);e.Next()
a.output.write_text(json.dumps(items,indent=2)+'\n');print(len(items),'native components');print(json.dumps(sorted(items,key=lambda x:max(x['size'])*sorted(x['size'])[1],reverse=True)[:18]))
