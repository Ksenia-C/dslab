import math
import pathlib
import argparse

args_parser = argparse.ArgumentParser()
args_parser.add_argument("--folder", type=str)
args = args_parser.parse_args()

folder = pathlib.Path(args.folder)

new_folder = folder.parent / f"{folder.name}_colored/"
if not new_folder.is_dir():
    new_folder.mkdir()

for path in folder.iterdir():
    if path.suffix != '.dot':
        continue
    max_sz = None
    min_sz = None
    with open(path, "r") as file:
        for line in file.readlines():
            if '[size=' in line:
                item_size = float(line.split('"')[1])
                if max_sz is None or max_sz < item_size:
                    max_sz = item_size
                if min_sz is None or min_sz > item_size:
                    min_sz = item_size
    max_sz = math.log(max_sz + 1)
    min_sz = math.log(min_sz + 1)
    with open(path, "r") as file:
        with open(new_folder / path.name, "w") as new_file:
            for line in file.readlines():
                if '[size=' in line:
                    item_size = float(line.split('"')[1])
                    item_size = math.log(item_size + 1)
                    red =int((item_size - min_sz) / (max_sz - min_sz) * 255)
                    blue = 255 - red
                    red = hex(red).upper()[2:]
                    blue = hex(blue).upper()[2:]
                    if len(red) == 1:
                        red = f'0{red}'
                    if len(blue) == 1:
                        blue = f'0{blue}'
                    print(f'{" ".join(line.split()[:-1])} [size="{item_size}",color="#{red}00{blue}"];', file=new_file)
                else:
                    print(line, file=new_file)