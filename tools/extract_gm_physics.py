#!/usr/bin/env python3
"""
Extract physics/collision data from the GameMaker project.

Reads all room .yy files and object/sprite .yy files, then outputs
MapPhysics.json — a structured file the server can use to validate
player movement (collision, springs, kill-zones, etc.).

Usage: python3 tools/extract_gm_physics.py [output_path]
  default output: target/release/MapPhysics.json
"""

import os
import re
import sys
import json
import math

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT_DIR   = os.path.join(SCRIPT_DIR, '..')
GAME_DIR   = os.path.join(ROOT_DIR, 'disaster2d')
OBJ_DIR    = os.path.join(GAME_DIR, 'objects')
SPR_DIR    = os.path.join(GAME_DIR, 'sprites')
ROOM_DIR   = os.path.join(GAME_DIR, 'rooms')

OUTPUT_PATH = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT_DIR, 'target', 'release', 'MapPhysics.json')

MAP_ROOMS = {
    0:  'room_hideandseek2',
    1:  'room_ravinemist',
    2:  'room_dotdotdot',
    3:  'room_deserttown',
    4:  'room_youcantrun',
    5:  'room_limpcity',
    6:  'room_notperfect',
    7:  'room_kindandfair',
    8:  'room_act9',
    9:  'room_nastyparadise',
    10: 'room_pricelessfreedom',
    11: 'room_volcanovalley',
    12: 'room_greenhill',
    13: 'room_majongforest',
    14: None,
    15: 'room_torturecave',
    16: 'room_dartower',
    17: 'room_haundream',
    18: 'room_marijuna',
    19: 'room_angelisland',
    20: 'room_fartzone',
    21: 'room_weedzone',
    22: 'room_youcantrun',
}

SPRING_DIRECTION = {
    'obj_spring_up':    'up',
    'obj_bspring_up':   'up',
    'obj_yspring_up':   'up',
    'obj_hd_spring':    'up',
    'obj_spring_left':  'left',
    'obj_bspring_left': 'left',
    'obj_spring_right': 'right',
    'obj_bspring_right':'right',
}

EXPLICIT_KILL = {
    'obj_deathtp',
    'obj_abyss',
    'obj_damage',
    'obj_act9_instakill',
    'obj_vv_lavacolumn',
    'obj_nap_icespike',
    'obj_suddendeath',
    'obj_spile_r',
}

SPECIAL_OBJECTS = {
    'obj_pf_lift':          'lift',
    'obj_kaf_speedboobster':'speed_boost',
    'obj_np_teleporn':      'teleport',
    'obj_weed_convejor':    'conveyor_left',
    'obj_weed_convejor2':   'conveyor_right',
    'obj_weed_convejor3':   'conveyor_left',
    'obj_weed_zone':        'kill',
}

DECORATION_PATTERNS = [
    'parallax', 'wave', 'bush', 'palm', 'cloud', 'fog', 'darkness',
    'jumpscare', 'ball', 'controller', 'spawner', 'spawn', 'corpse',
    'warning', 'sound', 'camera', 'config', 'trigger', 'dummy',
    'face', 'bodies', 'crystal', 'hide', 'vase', 'eggman', 'jack',
    'ghost', 'lantern', 'selfinsert', 'smokearea', 'chain', 'eye',
    'indicator', 'zipline', 'water', 'ghz_water', 'eggstatue',
    'misc', 'tile', 'mermer', 'guko', 'judger', 'crystalcontroller',
    'barrel', 'snowball_waypoint', 'background', 'npring', 'ringspawn',
    'ring_spawner', 'blackring', 'bigring', 'angleallower', 'angleanuller',
    'kapla', 'sisi', 'tailsdoll', 'deserttown_i', 'darktower_sisi',
]

def strip_trailing_commas(text):
    """Remove trailing commas before } or ] so json.loads works."""
    return re.sub(r',\s*([}\]])', r'\1', text)

def read_yy(path):
    """Read a .yy file and return as a dict (None on failure)."""
    try:
        with open(path, encoding='utf-8') as f:
            raw = f.read()
        return json.loads(strip_trailing_commas(raw))
    except Exception:
        return None

print("Reading objects...")
objects_raw = {}

for name in os.listdir(OBJ_DIR):
    yy_path = os.path.join(OBJ_DIR, name, name + '.yy')
    data = read_yy(yy_path)
    if data:
        objects_raw[name] = data

def get_parent(name):
    d = objects_raw.get(name)
    if d and d.get('parentObjectId'):
        return d['parentObjectId'].get('name')
    return None

def get_all_descendants(root):
    """Return all descendants of root (inclusive)."""
    result = {root}
    for name in list(objects_raw.keys()):
        cur = name
        visited = set()
        while cur and cur not in visited:
            visited.add(cur)
            if cur == root:
                result.add(name)
                break
            cur = get_parent(cur)
    return result

floor_descendants  = get_all_descendants('obj_floor_parent')
spring_descendants = get_all_descendants('obj_spring_parent')
platform_descendants = get_all_descendants('obj_platform_jumptrough')
slope_names = {n for n in floor_descendants if 'slope' in n or 'slop' in n}
conveyor_names = {n for n in floor_descendants if 'convejor' in n or 'conveyor' in n}

kill_platform_names = {n for n in floor_descendants if 'lava' in n or 'spike' in n or 'spile' in n}
kill_platform_names.update({n for n in floor_descendants if 'movingspike' in n})

print(f"  floor_parent descendants: {len(floor_descendants)}")
print(f"  spring_parent descendants: {len(spring_descendants)}")

print("Reading sprites...")
sprites = {}

for name in os.listdir(SPR_DIR):
    yy_path = os.path.join(SPR_DIR, name, name + '.yy')
    data = read_yy(yy_path)
    if not data:
        continue
    ox, oy = 0, 0
    seq = data.get('sequence') or {}
    tracks = seq.get('tracks') or []
    for tr in tracks:
        kf = (tr.get('keyframes') or {}).get('Keyframes') or []
        for k in kf:
            ch = k.get('Channels') or {}
            if '0' in ch:
                ch0 = ch['0']
                ox = ch0.get('x', 0)
                oy = ch0.get('y', 0)
    ox = seq.get('xorigin', ox)
    oy = seq.get('yorigin', oy)

    sprites[name] = {
        'width':       data.get('width', 0),
        'height':      data.get('height', 0),
        'bbox_left':   data.get('bbox_left', 0),
        'bbox_top':    data.get('bbox_top', 0),
        'bbox_right':  data.get('bbox_right', 0),
        'bbox_bottom': data.get('bbox_bottom', 0),
        'origin_x':    ox,
        'origin_y':    oy,
    }

print(f"  loaded {len(sprites)} sprites")

def classify_object(name):
    """Return (category, extra) for an object, or (None, None) if decoration."""
    if name in SPRING_DIRECTION:
        return ('spring', SPRING_DIRECTION[name])
    if name in spring_descendants and name != 'obj_spring_parent':
        return ('spring', 'up')

    if name in kill_platform_names and name in floor_descendants:
        return ('kill_platform', None)

    if name in EXPLICIT_KILL:
        return ('kill', None)

    if name in conveyor_names:
        return ('conveyor', SPECIAL_OBJECTS.get(name, 'conveyor'))

    if name in SPECIAL_OBJECTS:
        return (SPECIAL_OBJECTS[name], None)

    if name in slope_names:
        return ('slope', None)

    if name in platform_descendants and name != 'obj_floor_parent':
        return ('platform', None)

    if name in floor_descendants and name != 'obj_floor_parent':
        return ('solid', None)

    low = name.lower()
    for pat in DECORATION_PATTERNS:
        if pat in low:
            return (None, None)

    return (None, None)

def rotated_aabb(corners_local):
    """Return (min_x, min_y, max_x, max_y) of a set of local (x,y) points."""
    xs = [c[0] for c in corners_local]
    ys = [c[1] for c in corners_local]
    return min(xs), min(ys), max(xs), max(ys)

def compute_hitbox(obj_name, ix, iy, scale_x, scale_y, rotation_deg):
    """
    Compute world AABB from object position, scale, rotation and sprite bbox.
    Returns dict with x, y, w, h (top-left + dimensions).
    """
    obj_data = objects_raw.get(obj_name, {})
    sprite_ref = obj_data.get('spriteId') or obj_data.get('spriteMaskId')
    if isinstance(sprite_ref, dict):
        spr_name = sprite_ref.get('name')
    else:
        spr_name = None

    if not spr_name:
        par = get_parent(obj_name)
        if par:
            par_data = objects_raw.get(par, {})
            spr_ref2 = par_data.get('spriteId')
            if isinstance(spr_ref2, dict):
                spr_name = spr_ref2.get('name')

    spr = sprites.get(spr_name) if spr_name else None
    if not spr:
        return {'x': ix - 1, 'y': iy - 1, 'w': 2, 'h': 2, 'sprite': None}

    ox = spr['origin_x']
    oy = spr['origin_y']
    bl = spr['bbox_left']
    bt = spr['bbox_top']
    br = spr['bbox_right']
    bb = spr['bbox_bottom']

    corners_sprite = [
        (bl - ox, bt - oy),
        (br + 1 - ox, bt - oy),
        (bl - ox, bb + 1 - oy),
        (br + 1 - ox, bb + 1 - oy),
    ]

    corners_scaled = [(cx * scale_x, cy * scale_y) for cx, cy in corners_sprite]

    if rotation_deg != 0.0:
        rad = math.radians(rotation_deg)
        cos_r = math.cos(rad)
        sin_r = math.sin(rad)
        corners_rotated = [
            (cx * cos_r - cy * sin_r,
             cx * sin_r + cy * cos_r)
            for cx, cy in corners_scaled
        ]
    else:
        corners_rotated = corners_scaled

    min_x, min_y, max_x, max_y = rotated_aabb(corners_rotated)

    return {
        'x': round(ix + min_x, 4),
        'y': round(iy + min_y, 4),
        'w': round(max_x - min_x, 4),
        'h': round(max_y - min_y, 4),
        'sprite': spr_name,
    }

print("Parsing rooms...")

def parse_room(room_name):
    yy_path = os.path.join(ROOM_DIR, room_name, room_name + '.yy')
    data = read_yy(yy_path)
    if not data:
        return None

    rs = data.get('roomSettings', {})
    room_width  = rs.get('Width', 0)
    room_height = rs.get('Height', 0)

    instances_out = []
    layers = data.get('layers') or []

    def process_layer(layer):
        for inst in layer.get('instances') or []:
            obj_ref = inst.get('objectId')
            if not isinstance(obj_ref, dict):
                continue
            obj_name = obj_ref.get('name', '')
            if not obj_name:
                continue

            category, extra = classify_object(obj_name)
            if category is None:
                continue

            ix = inst.get('x', 0.0)
            iy = inst.get('y', 0.0)
            sx = inst.get('scaleX', 1.0)
            sy = inst.get('scaleY', 1.0)
            rot = inst.get('rotation', 0.0)

            hitbox = compute_hitbox(obj_name, ix, iy, sx, sy, rot)

            entry = {
                'id':       inst.get('name', ''),
                'object':   obj_name,
                'category': category,
                'x': round(ix, 2),
                'y': round(iy, 2),
                'scale_x': sx,
                'scale_y': sy,
                'rotation': rot,
                'hitbox': hitbox,
            }
            if extra is not None:
                entry['extra'] = extra

            instances_out.append(entry)

        for sub in layer.get('layers') or []:
            process_layer(sub)

    for layer in layers:
        process_layer(layer)

    return {
        'width':     room_width,
        'height':    room_height,
        'instances': instances_out,
    }

obj_catalogue = {}
for name in sorted(objects_raw.keys()):
    category, extra = classify_object(name)
    if category is None:
        continue
    obj_data = objects_raw[name]
    sprite_ref = obj_data.get('spriteId')
    spr_name = sprite_ref.get('name') if isinstance(sprite_ref, dict) else None
    spr_info = sprites.get(spr_name) if spr_name else None

    entry = {
        'category': category,
        'parent': get_parent(name),
    }
    if extra is not None:
        entry['extra'] = extra
    if spr_info:
        entry['sprite'] = {'name': spr_name, **spr_info}
    obj_catalogue[name] = entry

rooms_out = {}
for room_name in sorted(os.listdir(ROOM_DIR)):
    room_data = parse_room(room_name)
    if room_data is None:
        continue
    if not room_data['instances']:
        continue
    rooms_out[room_name] = room_data
    print(f"  {room_name}: {len(room_data['instances'])} interactive instances "
          f"({room_data['width']}x{room_data['height']})")

map_index = {}
for map_id, room_name in MAP_ROOMS.items():
    if room_name and room_name in rooms_out:
        map_index[str(map_id)] = room_name

output = {
    'version': 1,
    'map_index': map_index,
    'objects':  obj_catalogue,
    'rooms':    rooms_out,
}

os.makedirs(os.path.dirname(OUTPUT_PATH), exist_ok=True)
with open(OUTPUT_PATH, 'w', encoding='utf-8') as f:
    json.dump(output, f, indent=2, ensure_ascii=False)

total_instances = sum(len(r['instances']) for r in rooms_out.values())
cat_counts = {}
for r in rooms_out.values():
    for inst in r['instances']:
        cat_counts[inst['category']] = cat_counts.get(inst['category'], 0) + 1

print(f"\nDone! Output: {OUTPUT_PATH}")
print(f"  {len(rooms_out)} rooms, {total_instances} total interactive instances")
print(f"  {len(obj_catalogue)} interactive object types")
print("  Categories:")
for cat, cnt in sorted(cat_counts.items()):
    print(f"    {cat}: {cnt}")
