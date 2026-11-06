import os, sys
print('CM_PATH',sys.path)
print('CM_FLASH',os.listdir('/flash'))
print('CM_LIBS',os.listdir('/flash/libs'))
try:
    import M5Things
    print('CM_M5THINGS_PRESENT')
except ImportError as error:
    print('CM_M5THINGS_ABSENT', str(error))
