use super::tree::CommandTree;

pub fn smart_usage(tree: &CommandTree, parent: usize) -> Vec<String> {
    let optional = tree.node(parent).executable;
    let mut out = Vec::new();
    for child in &tree.node(parent).children {
        if tree.node(*child).is_literal() {
            continue;
        }
        if let Some(usage) = smart_usage_recursive(tree, *child, optional, false, 0) {
            out.push(usage);
        }
    }
    out
}

fn smart_usage_recursive(
    tree: &CommandTree,
    node: usize,
    optional: bool,
    deep: bool,
    depth: usize,
) -> Option<String> {
    if depth > 16 {
        return None;
    }
    let n = tree.node(node);
    let this = if optional {
        format!("[{}]", n.usage_text())
    } else {
        n.usage_text()
    };
    let child_optional = n.executable;
    let (open, close) = if child_optional {
        ("[", "]")
    } else {
        ("(", ")")
    };

    if deep {
        return Some(this);
    }

    if let Some(redirect) = n.redirect {
        let target = if redirect == tree.root {
            "...".to_string()
        } else {
            format!("-> {}", tree.node(redirect).usage_text())
        };
        return Some(format!("{this} {target}"));
    }

    match n.children.len() {
        0 => {}
        1 => {
            let usage = smart_usage_recursive(
                tree,
                n.children[0],
                child_optional,
                child_optional,
                depth + 1,
            );
            if let Some(usage) = usage {
                return Some(format!("{this} {usage}"));
            }
        }
        _ => {
            let mut child_usage: Vec<String> = Vec::new();
            for child in &n.children {
                if let Some(usage) =
                    smart_usage_recursive(tree, *child, child_optional, true, depth + 1)
                    && !child_usage.contains(&usage)
                {
                    child_usage.push(usage);
                }
            }
            match child_usage.len() {
                0 => {}
                1 => {
                    let usage = child_usage.remove(0);
                    let usage = if child_optional {
                        format!("[{usage}]")
                    } else {
                        usage
                    };
                    return Some(format!("{this} {usage}"));
                }
                _ => {
                    let joined: Vec<String> = n
                        .children
                        .iter()
                        .map(|c| tree.node(*c).usage_text())
                        .collect();
                    return Some(format!("{this} {open}{}{close}", joined.join("|")));
                }
            }
        }
    }
    Some(this)
}
