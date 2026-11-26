"""Explicit original-photo alpha masks and projective paper rectification, no network."""
from pathlib import Path
import hashlib
import numpy as np
from PIL import Image, ImageDraw
import cv2

def projected_texture(source, spec, output):
    data=Path(source).read_bytes()
    if hashlib.sha256(data).hexdigest()!=spec['sourceSha256']:
        raise ValueError('Pinned exact-SKU photograph changed')
    im=Image.open(source).convert('RGBA');factor=4
    alpha=Image.new('L',(im.width*factor,im.height*factor),0);draw=ImageDraw.Draw(alpha)
    mask=spec['alphaMask']
    for poly in mask.get('includePolygons',[]):draw.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=255)
    for poly in mask.get('excludePolygons',[]):draw.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=0)
    for ellipse in mask.get('excludeEllipses',[]):draw.ellipse([round(v*factor)for v in ellipse],fill=0)
    im.putalpha(alpha.resize(im.size,Image.Resampling.LANCZOS))
    quad=np.asarray(spec['perspectiveQuad'],dtype=np.float32);width,height=spec['rectifiedPixels']
    if quad.shape!=(4,2)or not np.isfinite(quad).all()or not(1<=width<=4096 and 1<=height<=4096):raise ValueError('Invalid explicit projection')
    if sum(quad[i,0]*quad[(i+1)%4,1]-quad[(i+1)%4,0]*quad[i,1] for i in range(4))<=0:raise ValueError('Mirrored paper registration')
    transform=cv2.getPerspectiveTransform(quad,np.asarray([[0,0],[width-1,0],[width-1,height-1],[0,height-1]],dtype=np.float32))
    result=Image.fromarray(cv2.warpPerspective(np.asarray(im),transform,(width,height),flags=cv2.INTER_LANCZOS4,borderMode=cv2.BORDER_CONSTANT,borderValue=(0,0,0,0)))
    result.save(output,optimize=True)
    return {**spec,'outputPixels':[width,height],'method':'Explicit exact-SKU photograph paper polygons and hardware exclusions at 4x supersampling, then original-image corner homography/Lanczos4 rectification. No generated lettering or shell pixels; estimated registration is retained.'}
