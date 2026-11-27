pub(crate) fn is_cyrillic(c: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&c)
}

pub(crate) fn to_qwerty(s: &str) -> String {
    if !s.chars().any(is_cyrillic) {
        return s.to_string();
    }
    s.chars().map(key).collect()
}

fn key(c: char) -> char {
    match c {
        'й' => 'q',
        'ц' => 'w',
        'у' => 'e',
        'к' => 'r',
        'е' => 't',
        'н' => 'y',
        'г' => 'u',
        'ш' => 'i',
        'щ' => 'o',
        'з' => 'p',
        'х' => '[',
        'ъ' => ']',
        'ф' => 'a',
        'ы' => 's',
        'в' => 'd',
        'а' => 'f',
        'п' => 'g',
        'р' => 'h',
        'о' => 'j',
        'л' => 'k',
        'д' => 'l',
        'ж' => ';',
        'э' => '\'',
        'я' => 'z',
        'ч' => 'x',
        'с' => 'c',
        'м' => 'v',
        'и' => 'b',
        'т' => 'n',
        'ь' => 'm',
        'б' => ',',
        'ю' => '.',
        'ё' => '`',
        'Й' => 'Q',
        'Ц' => 'W',
        'У' => 'E',
        'К' => 'R',
        'Е' => 'T',
        'Н' => 'Y',
        'Г' => 'U',
        'Ш' => 'I',
        'Щ' => 'O',
        'З' => 'P',
        'Х' => '{',
        'Ъ' => '}',
        'Ф' => 'A',
        'Ы' => 'S',
        'В' => 'D',
        'А' => 'F',
        'П' => 'G',
        'Р' => 'H',
        'О' => 'J',
        'Л' => 'K',
        'Д' => 'L',
        'Ж' => ':',
        'Э' => '"',
        'Я' => 'Z',
        'Ч' => 'X',
        'С' => 'C',
        'М' => 'V',
        'И' => 'B',
        'Т' => 'N',
        'Ь' => 'M',
        'Б' => '<',
        'Ю' => '>',
        'Ё' => '~',
        '.' => '/',
        ',' => '?',
        '"' => '@',
        '№' => '#',
        ';' => '$',
        ':' => '^',
        '?' => '&',
        '/' => '|',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_names_typed_on_the_russian_layout() {
        assert_eq!(to_qwerty("ещппду"), "toggle");
        assert_eq!(to_qwerty("ыуе"), "set");
        assert_eq!(to_qwerty("ьщвгдуы"), "modules");
        assert_eq!(to_qwerty("ЕЩППДУ"), "TOGGLE");
    }

    #[test]
    fn a_mixed_word_keeps_its_latin_half() {
        assert_eq!(to_qwerty("killфгкф"), "killaura");
        assert_eq!(to_qwerty("ыуе fly"), "set fly");
    }

    #[test]
    fn punctuation_moves_only_in_cyrillic_text() {
        assert_eq!(to_qwerty("ьштускфаеЖыещту"), "minecraft:stone");
        assert_eq!(to_qwerty("ёЁхъХЪжэЖЭбюБЮ"), "`~[]{};':\",.<>");
        assert_eq!(to_qwerty("ф.ф,\"№;:?/"), "a/a?@#$^&|");
    }

    #[test]
    fn text_with_no_cyrillic_is_unchanged() {
        for s in [
            "toggle",
            "set killaura range 4",
            "minecraft:stone",
            ".,;:?/\"",
            "",
            "42",
        ] {
            assert_eq!(to_qwerty(s), s);
        }
    }

    #[test]
    fn the_table_is_a_bijection_on_the_letters() {
        let letters = "йцукенгшщзхъфывапролджэячсмитьбюё";
        let upper: String = letters.chars().flat_map(char::to_uppercase).collect();
        let all: Vec<char> = letters.chars().chain(upper.chars()).map(key).collect();
        assert_eq!(all.len(), 66);
        assert!(all.iter().all(|c| c.is_ascii()), "{all:?}");
        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), all.len());
    }
}
