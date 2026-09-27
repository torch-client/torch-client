from typing import Any
from lib.utils import identifier_to_path, to_camel_case, get_dir_location

inventory_menus_dir = get_dir_location("../azalea-inventory/src/lib.rs")

def update_menus(initial_menu_entries: dict[str, Any]):
    new_menus: list[str] = [""] * len(initial_menu_entries)
    for menu_id, menu in initial_menu_entries.items():
        new_menus[menu["protocol_id"]] = menu_name_to_enum_name(menu_id)

    new_menus.insert(0, "Player")

    with open(inventory_menus_dir, "r") as f:
        menus_rs = f.read().splitlines()

        start_line_index = 0

        current_menus = []
        in_the_macro = False
        for i, line in enumerate(menus_rs):
            if line.startswith("declare_menus!"):
                in_the_macro = True
                start_line_index = i
            if in_the_macro:
                if line.startswith("    ") and line.endswith("{"):
                    current_menu = line[:-1].strip()
                    current_menus.append(current_menu)

        print("current_menus", current_menus)
        print("new_menus", new_menus)

        if current_menus != new_menus:
            current_menus_list_index = 0
            new_menus_list_index = 0
            insert_line_index = start_line_index + 1
            while True:
                if (
                    current_menus_list_index < len(current_menus)
                    and new_menus_list_index < len(new_menus)
                    and current_menus[current_menus_list_index]
                    == new_menus[new_menus_list_index]
                ):
                    current_menus_list_index += 1
                    new_menus_list_index += 1
                    while not menus_rs[insert_line_index].strip().startswith("}"):
                        insert_line_index += 1
                    insert_line_index += 1
                elif (
                    new_menus_list_index < len(new_menus)
                    and new_menus[new_menus_list_index] not in current_menus
                ):
                    menus_rs.insert(
                        insert_line_index,
                        f"    {new_menus[new_menus_list_index]} {{\n        todo!()\n    }},",
                    )
                    insert_line_index += 1
                    new_menus_list_index += 1
                    print(
                        "added",
                        current_menus_list_index,
                        new_menus_list_index,
                        insert_line_index,
                    )
                elif (
                    current_menus_list_index < len(current_menus)
                    and current_menus[current_menus_list_index] not in new_menus
                ):
                    while not menus_rs[insert_line_index].strip().startswith("}"):
                        menus_rs.pop(insert_line_index)
                    menus_rs.pop(insert_line_index)
                    current_menus_list_index += 1
                    print(
                        "removed",
                        current_menus_list_index,
                        new_menus_list_index,
                        insert_line_index,
                    )

                elif current_menus_list_index >= len(current_menus):
                    for i in range(new_menus_list_index, len(new_menus)):
                        menus_rs.insert(
                            insert_line_index,
                            f"    {new_menus[i]} {{\n        todo!()\n    }},",
                        )
                        insert_line_index += 1
                    print(
                        "current_menus_list_index overflowed",
                        current_menus_list_index,
                        new_menus_list_index,
                        insert_line_index,
                    )
                    break
                elif new_menus_list_index >= len(new_menus):
                    for _ in range(current_menus_list_index, len(current_menus)):
                        while not menus_rs[insert_line_index].strip().startswith("}"):
                            menus_rs.pop(insert_line_index)
                        menus_rs.pop(insert_line_index)
                    print(
                        "new_menus_list_index overflowed",
                        current_menus_list_index,
                        new_menus_list_index,
                        insert_line_index,
                    )
                    break
    with open(inventory_menus_dir, "w") as f:
        f.write("\n".join(menus_rs))

def menu_name_to_enum_name(menu_name: str) -> str:
    return to_camel_case(identifier_to_path(menu_name))
