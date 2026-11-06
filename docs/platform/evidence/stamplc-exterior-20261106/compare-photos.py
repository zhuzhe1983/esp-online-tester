"""After the browser fixture, run from repository root with tools/requirements-model.txt."""
from pathlib import Path
import json,hashlib,shutil
import numpy as np,cv2
from PIL import Image,ImageDraw
A=Path('web/platform/assets/m5stack-k141');O=Path('output/playwright/stamplc-refresh');E=Path('docs/platform/evidence/stamplc-exterior-20261106');m=json.load(open(A/'model.json'));sources={x['id']:x for x in m['provenance']['sources']};receipt=json.load(open(E/'browser-check.json'));photos={'front':'product-photo-d0a96a17d6','back':'product-photo-78481c3a59','right':'product-photo-52c55c8576','left':'product-photo-6e04e7ced2','top':'product-photo-17c5941ff1','bottom':'product-photo-db1831c2d3'}
def silhouette(im):
 mask=(np.asarray(im).min(axis=2)<245).astype('uint8');contours,_=cv2.findContours(mask,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);out=np.zeros_like(mask);cv2.drawContours(out,[max(contours,key=cv2.contourArea)],-1,1,-1);return out.astype(bool)
rows=[];grid=Image.new('RGB',(1200,6*430),'white');draw=ImageDraw.Draw(grid)
for i,(view,sid)in enumerate(photos.items()):
 photo=Image.open(A/('reference-'+sources[sid]['archiveName'])).convert('RGB').resize((800,800),Image.Resampling.LANCZOS);render=Image.open(O/f'final-{view}.png').convert('RGB');mask=Image.open(O/f'final-{view}-mask.png').convert('RGB');a=silhouette(photo);b=silhouette(mask);iou=float((a&b).sum()/(a|b).sum());over=Image.blend(photo,render,.5)
 row=Image.new('RGB',(2400,840),'white');rd=ImageDraw.Draw(row)
 for j,(im,title)in enumerate([(photo,'Official photo'),(render,'Current model'),(over,'50% overlay')]):row.paste(im,(j*800,40));rd.text((j*800+20,14),view+' / '+title,fill='black');grid.paste(im.resize((400,400)),(j*400,i*430+30));draw.text((j*400+10,i*430+10),view+' / '+title,fill='black')
 row.save(E/f'{view}-comparison.webp',lossless=True)
 shutil.copy(O/f'final-{view}.png',E/f'{view}.png');shutil.copy(O/f'final-{view}-mask.png',E/f'{view}-mask.png')
 rows.append({'view':view,'sourceId':sid,'sourceSha256':sources[sid]['sha256'],'renderSha256':hashlib.sha256((E/f'{view}.png').read_bytes()).hexdigest(),'silhouetteIoU':iou,'status':'qualitative-only','camera':next(x['camera']for x in receipt['images']if x['view']==view),'method':'Source resized to 800x800, min RGB<245, largest external contour filled; compare with independent renderer silhouette. Shadows/lighting/CAD revision and small source perspective differ. IoU is reported without a photo-identical acceptance claim.'})
grid.save(E/'six-view-comparison.webp',lossless=True)
(E/'photo-comparison.json').write_text(json.dumps({'modelSha256':receipt['modelSha256'],'views':rows,'scope':'Six available orthographic/near-orthographic exterior photographs; not manufacturing/optical calibration.'},indent=2)+'\n')
print([(x['view'],round(x['silhouetteIoU'],4))for x in rows])
