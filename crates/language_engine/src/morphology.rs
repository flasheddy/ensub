use std::collections::HashSet;

/// Returns deterministic WordNet-style lemma candidates for an English form.
pub fn lemma_candidates(surface: &str) -> Vec<String> {
    let surface = surface.trim().to_lowercase();
    if surface.is_empty() {
        return Vec::new();
    }

    if let Some(lemma) = irregular_lemma(&surface) {
        let mut candidates = vec![lemma.to_string()];
        if lemma != surface {
            candidates.push(surface);
        }
        return candidates;
    }

    let mut candidates = vec![surface.clone()];
    if let Some(stem) = surface.strip_suffix("ies").filter(|stem| !stem.is_empty()) {
        candidates.push(format!("{stem}y"));
    }
    if let Some(stem) = surface.strip_suffix("ied").filter(|stem| !stem.is_empty()) {
        candidates.push(format!("{stem}y"));
    }
    if let Some(stem) = surface.strip_suffix("ing").filter(|stem| stem.len() >= 2) {
        push_verb_stems(&mut candidates, stem);
    }
    if let Some(stem) = surface.strip_suffix("ed").filter(|stem| stem.len() >= 2) {
        push_verb_stems(&mut candidates, stem);
    }
    if let Some(stem) = surface.strip_suffix("es").filter(|stem| stem.len() >= 2) {
        candidates.push(stem.to_string());
    }
    if let Some(stem) = surface.strip_suffix('s').filter(|stem| stem.len() >= 2) {
        candidates.push(stem.to_string());
    }

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.clone()));
    candidates
}

fn push_verb_stems(candidates: &mut Vec<String>, stem: &str) {
    candidates.push(stem.to_string());
    candidates.push(format!("{stem}e"));
    if let Some(shortened) = remove_doubled_consonant(stem) {
        candidates.push(shortened);
    }
}

fn remove_doubled_consonant(value: &str) -> Option<String> {
    let mut characters = value.chars().rev();
    let last = characters.next()?;
    let previous = characters.next()?;
    if last == previous
        && last.is_ascii_alphabetic()
        && !matches!(last, 'a' | 'e' | 'i' | 'o' | 'u')
    {
        let shortened = &value[..value.len() - last.len_utf8()];
        Some(shortened.to_string())
    } else {
        None
    }
}

fn irregular_lemma(surface: &str) -> Option<&'static str> {
    match surface {
        "am" | "are" | "been" | "being" | "is" | "was" | "were" => Some("be"),
        "better" | "best" => Some("good"),
        "bought" => Some("buy"),
        "brought" => Some("bring"),
        "came" => Some("come"),
        "children" => Some("child"),
        "did" | "done" => Some("do"),
        "drank" | "drunk" => Some("drink"),
        "ate" | "eaten" => Some("eat"),
        "feet" => Some("foot"),
        "geese" => Some("goose"),
        "gone" | "went" => Some("go"),
        "got" | "gotten" => Some("get"),
        "had" | "has" => Some("have"),
        "made" => Some("make"),
        "men" => Some("man"),
        "mice" => Some("mouse"),
        "ran" => Some("run"),
        "said" => Some("say"),
        "saw" | "seen" => Some("see"),
        "spoke" | "spoken" => Some("speak"),
        "teeth" => Some("tooth"),
        "thought" => Some("think"),
        "took" | "taken" => Some("take"),
        "women" => Some("woman"),
        "wrote" | "written" => Some("write"),
        _ => None,
    }
}
