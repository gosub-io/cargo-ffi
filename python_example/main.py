import ctypes
import os

print("Hello, this is a Python example using the Gosub engine!")

libname = "libgosub_engine.so"

lib_path = os.path.join("..", "target", "release", libname)
lib = ctypes.CDLL(lib_path)

class GosubEngineHandle(ctypes.Structure):
    _fields_ = [("_0", ctypes.c_void_p)]

lib.gosub_engine_new.restype = GosubEngineHandle
lib.gosub_load_url.argtypes = [GosubEngineHandle, ctypes.c_char_p]
lib.gosub_tick.argtypes = [GosubEngineHandle]
lib.gosub_tick.restype = ctypes.c_bool
lib.gosub_render.argtypes = [GosubEngineHandle, ctypes.POINTER(ctypes.c_uint8), ctypes.c_size_t]
lib.gosub_render.restype = ctypes.c_size_t
lib.gosub_engine_free.argtypes = [GosubEngineHandle]

engine = lib.gosub_engine_new()
lib.gosub_load_url(engine, b"https://example.com")

for i in range(10):
    if lib.gosub_tick(engine):
        buf = (ctypes.c_uint8 * 4)()
        lib.gosub_render(engine, buf, 4)
        print(f"Pixel {i}: {list(buf)}")

lib.gosub_engine_free(engine)