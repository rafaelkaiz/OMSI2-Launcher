//! Launcher translations. The Spanish text of each string is its key; `tr` returns the text in the current language and falls back
//! to the Spanish original when a string has no translation. Strings with `{}` are templates filled in with `trf`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

/// A language, named by the three letters OMSI uses for it in `Languages\<CODE>_basic.olf` (ESP, ENG, DEU...).
///
/// Nine languages are built into the launcher. Any other one appears in the list when it has a file `%APPDATA%OMSI2-Launcherlang<CODE>.txt`
/// (see `write_template`), and its launcher texts come from that file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Lang(&'static str);

#[allow(non_upper_case_globals)]
impl Lang {
    pub const Es: Lang = Lang("ESP");
    pub const En: Lang = Lang("ENG");
    pub const De: Lang = Lang("DEU");
    pub const Pl: Lang = Lang("POL");
    pub const Cs: Lang = Lang("CZE");
    pub const Fr: Lang = Lang("FRA");
    pub const Pt: Lang = Lang("POR");
    pub const Hu: Lang = Lang("HUN");
    pub const Sr: Lang = Lang("SRB");
    /// The languages the launcher is translated into by itself.
    pub const BUILTIN: [Lang; 9] = [Lang::Es, Lang::En, Lang::De, Lang::Pl, Lang::Cs, Lang::Fr, Lang::Pt, Lang::Hu, Lang::Sr];

    /// The three-letter code, in capitals.
    pub fn code(self) -> &'static str {
        self.0
    }

    /// The language with that code. The two-letter codes of older settings files (`es`, `de`...) are understood too.
    pub fn from_code(s: &str) -> Lang {
        let s = s.trim().to_uppercase();
        let legacy = match s.as_str() {
            "ES" => "ESP",
            "EN" => "ENG",
            "DE" => "DEU",
            "PL" => "POL",
            "CS" => "CZE",
            "FR" => "FRA",
            "PT" => "POR",
            other => other,
        };
        if legacy.is_empty() {
            return Lang::Es;
        }
        Lang::BUILTIN.into_iter().find(|l| l.0 == legacy).unwrap_or_else(|| Lang(intern(legacy)))
    }

    /// Name of the language in that language.
    pub fn name(self) -> &'static str {
        match self.0 {
            "ESP" => "Español",
            "ENG" => "English",
            "DEU" => "Deutsch",
            "POL" => "Polski",
            "CZE" => "Čeština",
            "FRA" => "Français",
            "POR" => "Português",
            "HUN" => "Magyar",
            "SRB" => "Srpski",
            code => external_name(code).unwrap_or(code),
        }
    }

    /// Column of the built-in table; `None` for Spanish, whose text is the key itself. A language without a column uses English.
    fn column(self) -> Option<usize> {
        match self.0 {
            "ESP" => None,
            "ENG" => Some(0),
            "DEU" => Some(1),
            "POL" => Some(2),
            "CZE" => Some(3),
            "FRA" => Some(4),
            "POR" => Some(5),
            _ => Some(0),
        }
    }
}

/// Codes seen so far, kept for the whole run so `Lang` can stay a cheap copyable value.
fn intern(code: &str) -> &'static str {
    static CODES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut codes = CODES.lock().unwrap();
    if let Some(c) = codes.iter().find(|c| **c == code) {
        return c;
    }
    let leaked: &'static str = Box::leak(code.to_string().into_boxed_str());
    codes.push(leaked);
    leaked
}

type Texts = HashMap<&'static str, &'static str>;

struct Current {
    lang: Lang,
    /// Texts read from the language's file in the `lang` folder; they win over the built-in ones.
    external: Texts,
}

fn current() -> &'static RwLock<Current> {
    static C: OnceLock<RwLock<Current>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(Current { lang: Lang::Es, external: Texts::new() }))
}

fn names() -> &'static Mutex<HashMap<String, &'static str>> {
    static N: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    N.get_or_init(|| Mutex::new(HashMap::new()))
}

fn external_name(code: &str) -> Option<&'static str> {
    if let Some(n) = names().lock().unwrap().get(code) {
        return Some(n);
    }
    let (_, name) = read_external(code);
    let name: &'static str = Box::leak(name?.into_boxed_str());
    names().lock().unwrap().insert(code.to_string(), name);
    Some(name)
}

/// Where the translation files are kept: `%APPDATA%\OMSI2-Launcher\lang`.
pub fn lang_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("OMSI2-Launcher").join("lang")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

/// Reads `<CODE>.txt`: `@name=` gives the language's own name, then pairs of lines, `> original (Spanish)` and `= translation`.
fn read_external(code: &str) -> (Texts, Option<String>) {
    let mut texts = Texts::new();
    let mut name = None;
    let Ok(bytes) = std::fs::read(lang_dir().join(format!("{code}.txt"))) else { return (texts, name) };
    let text = String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes)).into_owned();
    let mut pending: Option<&'static str> = None;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(n) = line.strip_prefix("@name=") {
            name = Some(n.trim().to_string());
        } else if let Some(k) = line.strip_prefix("> ") {
            // only keys the launcher really has are kept, so a mistyped one cannot hide anything
            pending = ROWS.iter().find(|(key, _)| *key == unescape(k)).map(|(key, _)| *key);
        } else if let Some(v) = line.strip_prefix("= ") {
            if let (Some(k), false) = (pending.take(), v.trim().is_empty()) {
                texts.insert(k, Box::leak(unescape(v).into_boxed_str()) as &'static str);
            }
        }
    }
    (texts, name)
}

pub fn set_lang(l: Lang) {
    let (external, _) = read_external(l.code());
    *current().write().unwrap() = Current { lang: l, external };
}

pub fn lang() -> Lang {
    current().read().unwrap().lang
}

type Row = (&'static str, [&'static str; 6]);

fn table() -> &'static HashMap<&'static str, [&'static str; 6]> {
    static T: OnceLock<HashMap<&'static str, [&'static str; 6]>> = OnceLock::new();
    T.get_or_init(|| ROWS.iter().copied().collect())
}


/// The translations of the languages that are not a column of `ROWS`: one text per line, in the order of `ROWS`.
fn embedded(code: &str) -> Option<&'static Texts> {
    static HUN: OnceLock<Texts> = OnceLock::new();
    static SRB: OnceLock<Texts> = OnceLock::new();
    let (cell, source) = match code {
        "HUN" => (&HUN, include_str!("lang/HUN.txt")),
        "SRB" => (&SRB, include_str!("lang/SRB.txt")),
        _ => return None,
    };
    Some(cell.get_or_init(|| {
        let lines: Vec<&str> = source.lines().map(|l| l.trim_end_matches('\r')).collect();
        ROWS.iter().zip(lines).map(|((key, _), text)| (*key, Box::leak(unescape(text).into_boxed_str()) as &'static str)).collect()
    }))
}
pub fn has(key: &str) -> bool {
    table().contains_key(key)
}

/// The text for `key` in the current language.
pub fn tr(key: &str) -> &str {
    let cur = current().read().unwrap();
    if let Some(s) = cur.external.get(key) {
        return *s;
    }
    if let Some(s) = embedded(cur.lang.code()).and_then(|t| t.get(key)) {
        return *s;
    }
    match cur.lang.column().and_then(|i| table().get(key).map(|r| r[i])) {
        Some(s) => s,
        None => key,
    }
}

/// The codes of the languages found in OMSI's `Languages` folder (a `<CODE>_basic.olf` file each), in alphabetical order.
pub fn omsi_languages(omsi: &Path) -> Vec<Lang> {
    let mut codes: Vec<String> = std::fs::read_dir(omsi.join("Languages"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_uppercase();
            let code = name.strip_suffix("_BASIC.OLF")?;
            (code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic())).then(|| code.to_string())
        })
        .collect();
    codes.sort();
    codes.dedup();
    codes.iter().map(|c| Lang::from_code(c)).collect()
}

/// Every language the launcher can be shown in: the ones it is translated into and those with a file in the `lang` folder. OMSI's own
/// languages are not offered unless the launcher has texts for them.
pub fn available(_omsi: Option<&Path>) -> Vec<Lang> {
    let mut all: Vec<Lang> = Lang::BUILTIN.to_vec();
    if let Ok(rd) = std::fs::read_dir(lang_dir()) {
        for e in rd.flatten() {
            let file = e.file_name().to_string_lossy().to_uppercase();
            if let Some(code) = file.strip_suffix(".TXT") {
                if code.len() == 3 && code.chars().all(|c| c.is_ascii_alphabetic()) {
                    all.push(Lang::from_code(code));
                }
            }
        }
    }
    all.sort_by_key(|l| l.name().to_lowercase());
    all.dedup();
    all
}

/// Writes `lang\TEMPLATE.txt`: every text of the launcher with its English version, ready to be translated. Saved under the code of
/// the new language (for example `CHN.txt`), the language shows up in the launcher's list.
pub fn write_template() -> Result<PathBuf, String> {
    let dir = lang_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut out = String::from(
        "# Translation of the OMSI 2 Launcher. Translate the lines that start with \"= \" (they hold the English text now).\n\
         # Save the file as <CODE>.txt, with the same three letters OMSI uses for the language (CHN.txt for Languages\\CHN_basic.olf),\n\
         # in UTF-8. Lines that start with \"> \" are the original keys: do not change them. Keep every {} where it is.\n\
         # A line left as it is, or removed, simply shows the English text.\n\
         @name=The name of the language, written in that language\n\n",
    );
    for (key, tr) in ROWS {
        out.push_str(&format!("> {}\n= {}\n\n", escape(key), escape(tr[0])));
    }
    let path = dir.join("TEMPLATE.txt");
    std::fs::write(&path, out).map_err(|e| e.to_string())?;
    Ok(path)
}

/// `tr` for a template: each `{}` is replaced, in order, by the next argument.
pub fn trf(key: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = tr(key);
    for a in args {
        match rest.split_once("{}") {
            Some((head, tail)) => {
                out.push_str(head);
                out.push_str(a);
                rest = tail;
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

// Order of the translations: English, German, Polish, Czech, French, Portuguese.
const ROWS: &[Row] = &[
    // ---- start screen
    ("Listo.", ["Ready.", "Bereit.", "Gotowe.", "Připraveno.", "Prêt.", "Pronto."]),
    ("No encuentro OMSI 2. Pasa su carpeta como argumento o en OMSI_DIR.", [
        "OMSI 2 not found. Pass its folder as an argument or in OMSI_DIR.",
        "OMSI 2 nicht gefunden. Ordner als Argument oder in OMSI_DIR angeben.",
        "Nie znaleziono OMSI 2. Podaj jego folder jako argument lub w OMSI_DIR.",
        "OMSI 2 nenalezeno. Zadejte jeho složku jako argument nebo v OMSI_DIR.",
        "OMSI 2 introuvable. Indiquez son dossier en argument ou dans OMSI_DIR.",
        "OMSI 2 não encontrado. Indique a pasta como argumento ou em OMSI_DIR.",
    ]),
    ("¿Soporte? ¿Addons? ¿Info? forum.omnibussimulator.de/forum/", [
        "Support? Add-ons? Info? forum.omnibussimulator.de/forum/",
        "Support? Addons? Infos? forum.omnibussimulator.de/forum/",
        "Wsparcie? Dodatki? Info? forum.omnibussimulator.de/forum/",
        "Podpora? Doplňky? Info? forum.omnibussimulator.de/forum/",
        "Support ? Add-ons ? Infos ? forum.omnibussimulator.de/forum/",
        "Suporte? Add-ons? Info? forum.omnibussimulator.de/forum/",
    ]),
    ("Conductor actual:", ["Current driver:", "Aktueller Fahrer:", "Bieżący kierowca:", "Aktuální řidič:", "Conducteur actuel :", "Condutor atual:"]),
    ("Editar conductor", ["Edit driver", "Fahrer bearbeiten", "Edytuj kierowcę", "Upravit řidiče", "Modifier le conducteur", "Editar condutor"]),
    ("Crear nuevo conductor", ["Create new driver", "Neuen Fahrer erstellen", "Utwórz nowego kierowcę", "Vytvořit nového řidiče", "Créer un nouveau conducteur", "Criar novo condutor"]),
    ("Cierra OMSI 2 para editar conductores.", [
        "Close OMSI 2 to edit drivers.",
        "Schließe OMSI 2, um Fahrer zu bearbeiten.",
        "Zamknij OMSI 2, aby edytować kierowców.",
        "Zavřete OMSI 2 pro úpravu řidičů.",
        "Fermez OMSI 2 pour modifier les conducteurs.",
        "Feche o OMSI 2 para editar condutores.",
    ]),
    ("Fecha y hora", ["Date and time", "Datum und Uhrzeit", "Data i godzina", "Datum a čas", "Date et heure", "Data e hora"]),
    ("Usar la fecha y hora de la situación (del mapa)", [
        "Use the situation's date and time (from the map)",
        "Datum und Uhrzeit der Situation verwenden (aus der Karte)",
        "Użyj daty i godziny sytuacji (z mapy)",
        "Použít datum a čas situace (z mapy)",
        "Utiliser la date et l'heure de la situation (de la carte)",
        "Usar a data e hora da situação (do mapa)",
    ]),
    ("Usar fecha/hora actuales:", [
        "Use current date/time:",
        "Aktuelles Datum/Uhrzeit verwenden:",
        "Użyj bieżącej daty/godziny:",
        "Použít aktuální datum/čas:",
        "Utiliser la date/heure actuelles :",
        "Usar data/hora atuais:",
    ]),
    ("Elegir fecha y hora:", [
        "Choose date and time:",
        "Datum und Uhrzeit wählen:",
        "Wybierz datę i godzinę:",
        "Vybrat datum a čas:",
        "Choisir la date et l'heure :",
        "Escolher data e hora:",
    ]),
    ("Hora", ["Time", "Uhrzeit", "Godzina", "Čas", "Heure", "Hora"]),
    ("Fecha", ["Date", "Datum", "Data", "Datum", "Date", "Data"]),
    ("Año", ["Year", "Jahr", "Rok", "Rok", "Année", "Ano"]),
    ("Manual", ["Manual", "Manuell", "Ręcznie", "Ručně", "Manuel", "Manual"]),
    ("Mañana", ["Morning", "Morgen", "Poranek", "Ráno", "Matin", "Manhã"]),
    ("Día", ["Day", "Tag", "Dzień", "Den", "Jour", "Dia"]),
    ("Tarde", ["Afternoon", "Nachmittag", "Popołudnie", "Odpoledne", "Après-midi", "Tarde"]),
    ("Noche", ["Night", "Nacht", "Noc", "Noc", "Nuit", "Noite"]),
    ("Hora:", ["Time:", "Uhrzeit:", "Godzina:", "Čas:", "Heure :", "Hora:"]),
    ("OMSI está abierto: el cambio se aplicará la próxima vez que lo abras.", [
        "OMSI is open: the change will apply the next time you open it.",
        "OMSI ist geöffnet: Die Änderung gilt beim nächsten Start.",
        "OMSI jest otwarte: zmiana zadziała przy następnym uruchomieniu.",
        "OMSI je spuštěno: změna se projeví při dalším spuštění.",
        "OMSI est ouvert : le changement s'appliquera à la prochaine ouverture.",
        "O OMSI está aberto: a alteração aplica-se da próxima vez que o abrir.",
    ]),
    ("Información sobre el tiempo seleccionado:", [
        "Information on selected time:",
        "Informationen zum gewählten Zeitpunkt:",
        "Informacje o wybranym czasie:",
        "Informace o vybraném čase:",
        "Informations sur l'heure sélectionnée :",
        "Informações sobre a hora selecionada:",
    ]),
    ("¡Iniciar!", ["Start!", "Starten!", "Uruchom!", "Spustit!", "Démarrer !", "Iniciar!"]),
    ("Tutoriales", ["Tutorials", "Tutorials", "Samouczki", "Tutoriály", "Tutoriels", "Tutoriais"]),
    ("Opciones", ["Options", "Optionen", "Opcje", "Možnosti", "Options", "Opções"]),
    ("Cerrar", ["Close", "Schließen", "Zamknij", "Zavřít", "Fermer", "Fechar"]),
    ("Mapa actual:", ["Current map:", "Aktuelle Karte:", "Bieżąca mapa:", "Aktuální mapa:", "Carte actuelle :", "Mapa atual:"]),
    ("Cargar último estado del mapa", [
        "Load last map state",
        "Letzten Kartenstand laden",
        "Wczytaj ostatni stan mapy",
        "Načíst poslední stav mapy",
        "Charger le dernier état de la carte",
        "Carregar o último estado do mapa",
    ]),
    ("Cargar mapa sin autobuses", [
        "Load map without buses",
        "Karte ohne Busse laden",
        "Wczytaj mapę bez autobusów",
        "Načíst mapu bez autobusů",
        "Charger la carte sans bus",
        "Carregar mapa sem autocarros",
    ]),
    ("Cargar situación guardada", [
        "Load saved situation",
        "Gespeicherte Situation laden",
        "Wczytaj zapisaną sytuację",
        "Načíst uloženou situaci",
        "Charger une situation enregistrée",
        "Carregar situação guardada",
    ]),
    ("Información sobre el mapa seleccionado:", [
        "Information about the selected map:",
        "Informationen zur gewählten Karte:",
        "Informacje o wybranej mapie:",
        "Informace o vybrané mapě:",
        "Informations sur la carte sélectionnée :",
        "Informações sobre o mapa selecionado:",
    ]),
    // ---- launcher settings
    ("Ajustes del launcher", ["Launcher settings", "Launcher-Einstellungen", "Ustawienia launchera", "Nastavení launcheru", "Paramètres du launcher", "Definições do launcher"]),
    ("Estos ajustes son del launcher; las opciones de OMSI 2 están en el botón Opciones.", [
        "These are the launcher's settings; OMSI 2's options are under the Options button.",
        "Das sind die Einstellungen des Launchers; die Optionen von OMSI 2 findest du unter Optionen.",
        "To ustawienia launchera; opcje OMSI 2 są pod przyciskiem Opcje.",
        "Toto jsou nastavení launcheru; možnosti OMSI 2 jsou pod tlačítkem Možnosti.",
        "Ce sont les réglages du launcher ; les options d'OMSI 2 sont sous le bouton Options.",
        "Estas são as definições do launcher; as opções do OMSI 2 estão no botão Opções.",
    ]),
    ("Idioma del launcher", ["Launcher language", "Sprache des Launchers", "Język launchera", "Jazyk launcheru", "Langue du launcher", "Idioma do launcher"]),
    ("Cerrar el launcher al iniciar OMSI", ["Close the launcher when OMSI starts", "Launcher schließen, wenn OMSI startet", "Zamknij launcher po uruchomieniu OMSI", "Zavřít launcher po spuštění OMSI", "Fermer le launcher au démarrage d'OMSI", "Fechar o launcher quando o OMSI iniciar"]),
    ("No se encontró OMSI 2", ["OMSI 2 was not found", "OMSI 2 wurde nicht gefunden", "Nie znaleziono OMSI 2", "OMSI 2 nebyl nalezen", "OMSI 2 est introuvable", "O OMSI 2 não foi encontrado"]),
    ("Indica la carpeta donde tienes OMSI 2 instalado (la que contiene Omsi.exe).", [
        "Choose the folder where OMSI 2 is installed (the one that contains Omsi.exe).",
        "Wähle den Ordner, in dem OMSI 2 installiert ist (der Ordner mit Omsi.exe).",
        "Wskaż folder, w którym zainstalowano OMSI 2 (ten z plikiem Omsi.exe).",
        "Vyber složku, kde je OMSI 2 nainstalován (tu, která obsahuje Omsi.exe).",
        "Indique le dossier où OMSI 2 est installé (celui qui contient Omsi.exe).",
        "Indica a pasta onde o OMSI 2 está instalado (a que contém o Omsi.exe).",
    ]),
    ("Examinar...", ["Browse...", "Durchsuchen...", "Przeglądaj...", "Procházet...", "Parcourir...", "Procurar..."]),
    ("Esa carpeta no contiene Omsi.exe.", [
        "That folder does not contain Omsi.exe.",
        "Dieser Ordner enthält keine Omsi.exe.",
        "W tym folderze nie ma pliku Omsi.exe.",
        "Tato složka neobsahuje Omsi.exe.",
        "Ce dossier ne contient pas Omsi.exe.",
        "Essa pasta não contém o Omsi.exe.",
    ]),
    ("Elige la carpeta de OMSI 2", ["Choose the OMSI 2 folder", "OMSI-2-Ordner wählen", "Wybierz folder OMSI 2", "Vyberte složku OMSI 2", "Choisir le dossier d'OMSI 2", "Escolher a pasta do OMSI 2"]),
    ("Eliminar el archivo \"closecheck\" cuando OMSI 2 falle", [
        "Delete the \"closecheck\" file when OMSI 2 crashes",
        "Die Datei \"closecheck\" löschen, wenn OMSI 2 abstürzt",
        "Usuń plik \"closecheck\" po awarii OMSI 2",
        "Smazat soubor \"closecheck\" po pádu OMSI 2",
        "Supprimer le fichier \"closecheck\" quand OMSI 2 plante",
        "Eliminar o ficheiro \"closecheck\" quando o OMSI 2 falhar",
    ]),
    ("Instala el plugin en Ajustes del launcher para poder iniciar OMSI 2.", [
        "Install the plugin in the launcher settings to be able to start OMSI 2.",
        "Installiere das Plugin in den Launcher-Einstellungen, um OMSI 2 starten zu können.",
        "Zainstaluj wtyczkę w ustawieniach launchera, aby móc uruchomić OMSI 2.",
        "Nainstaluj plugin v nastavení launcheru, abys mohl spustit OMSI 2.",
        "Installez le plugin dans les réglages du launcher pour pouvoir lancer OMSI 2.",
        "Instala o plugin nas definições do launcher para poder iniciar o OMSI 2.",
    ]),
    ("Aparecer en:", ["Appear at:", "Start bei:", "Start w:", "Začít u:", "Apparaître à :", "Iniciar em:"]),
    ("Establecer hora de la situación", ["Set time of situation", "Zeit der Situation verwenden", "Ustaw czas sytuacji", "Nastavit čas situace", "Régler l'heure de la situation", "Definir a hora da situação"]),
    ("Crear plantilla de traducción", ["Create translation template", "Übersetzungsvorlage erstellen", "Utwórz szablon tłumaczenia", "Vytvořit šablonu překladu", "Créer un modèle de traduction", "Criar modelo de tradução"]),
    ("Abrir carpeta de idiomas", ["Open languages folder", "Sprachordner öffnen", "Otwórz folder języków", "Otevřít složku jazyků", "Ouvrir le dossier des langues", "Abrir pasta de idiomas"]),
    ("Para añadir otro idioma, crea una plantilla de traducción y guárdala como <CÓDIGO>.txt en la carpeta de idiomas.", [
        "To add another language, create a translation template and save it as <CODE>.txt in the languages folder.",
        "Für eine weitere Sprache eine Übersetzungsvorlage erstellen und als <CODE>.txt im Sprachordner speichern.",
        "Aby dodać kolejny język, utwórz szablon tłumaczenia i zapisz go jako <KOD>.txt w folderze języków.",
        "Chcete-li přidat další jazyk, vytvořte šablonu překladu a uložte ji jako <KÓD>.txt do složky jazyků.",
        "Pour ajouter une autre langue, créez un modèle de traduction et enregistrez-le sous <CODE>.txt dans le dossier des langues.",
        "Para adicionar outro idioma, cria um modelo de tradução e guarda-o como <CÓDIGO>.txt na pasta de idiomas.",
    ]),
    ("Seleccionar hora y fecha...", ["Select time and date...", "Uhrzeit und Datum wählen...", "Wybierz godzinę i datę...", "Vyberte čas a datum...", "Choisir l'heure et la date...", "Escolher hora e data..."]),
    ("Fecha:", ["Date:", "Datum:", "Data:", "Datum:", "Date :", "Data:"]),
    ("Hora actual", ["Current time", "Aktuelle Zeit", "Obecna godzina", "Aktuální čas", "Heure actuelle", "Hora atual"]),
    ("Fecha actual", ["Current date", "Aktuelles Datum", "Obecna data", "Aktuální datum", "Date actuelle", "Data atual"]),
    ("Año actual", ["Current year", "Aktuelles Jahr", "Obecny rok", "Aktuální rok", "Année actuelle", "Ano atual"]),
    ("Día de la semana:", ["Day of the week:", "Wochentag:", "Dzień tygodnia:", "Den v týdnu:", "Jour de la semaine :", "Dia da semana:"]),
    ("Festivos:", ["Holidays:", "Feiertage:", "Święta:", "Svátky:", "Jours fériés :", "Feriados:"]),
    ("Cambiar fecha y hora...", ["Change date and time...", "Datum und Uhrzeit ändern...", "Zmień datę i godzinę...", "Změnit datum a čas...", "Changer la date et l'heure...", "Alterar data e hora..."]),
    ("la de la situación", ["the situation's", "die der Situation", "z sytuacji", "ze situace", "celle de la situation", "a da situação"]),
    ("Si no cambias la fecha, OMSI usa la de la situación del mapa.", [
        "If you do not change the date, OMSI uses the map situation's date.",
        "Wenn du das Datum nicht änderst, verwendet OMSI das der Situation der Karte.",
        "Jeśli nie zmienisz daty, OMSI użyje daty z sytuacji mapy.",
        "Pokud datum nezměníš, OMSI použije datum situace mapy.",
        "Si vous ne changez pas la date, OMSI utilise celle de la situation de la carte.",
        "Se não alterares a data, o OMSI usa a da situação do mapa.",
    ]),
    ("Lunes", ["Monday", "Montag", "Poniedziałek", "Pondělí", "Lundi", "Segunda-feira"]),
    ("Martes", ["Tuesday", "Dienstag", "Wtorek", "Úterý", "Mardi", "Terça-feira"]),
    ("Miércoles", ["Wednesday", "Mittwoch", "Środa", "Středa", "Mercredi", "Quarta-feira"]),
    ("Jueves", ["Thursday", "Donnerstag", "Czwartek", "Čtvrtek", "Jeudi", "Quinta-feira"]),
    ("Viernes", ["Friday", "Freitag", "Piątek", "Pátek", "Vendredi", "Sexta-feira"]),
    ("Sábado", ["Saturday", "Samstag", "Sobota", "Sobota", "Samedi", "Sábado"]),
    ("Domingo", ["Sunday", "Sonntag", "Niedziela", "Neděle", "Dimanche", "Domingo"]),
    ("Página {}", ["Page {}", "Seite {}", "Strona {}", "Strana {}", "Page {}", "Página {}"]),
    ("Esperando la ventana de posición inicial de OMSI...", [
        "Waiting for OMSI's start position window...",
        "Warte auf das Startpositionsfenster von OMSI...",
        "Czekam na okno pozycji początkowej OMSI...",
        "Čekám na okno počáteční pozice OMSI...",
        "En attente de la fenêtre de position initiale d'OMSI...",
        "A aguardar a janela de posição inicial do OMSI...",
    ]),
    ("Eligiendo el punto de aparición...", ["Choosing the spawn point...", "Startposition wird gewählt...", "Wybieranie punktu startowego...", "Vybírám výchozí místo...", "Choix du point d'apparition...", "A escolher o ponto de início..."]),
    ("OMSI se cerró antes de pedir la posición inicial.", [
        "OMSI closed before asking for the start position.",
        "OMSI wurde geschlossen, bevor es die Startposition abfragte.",
        "OMSI zamknął się, zanim zapytał o pozycję początkową.",
        "OMSI se zavřelo dřív, než se zeptalo na počáteční pozici.",
        "OMSI s'est fermé avant de demander la position initiale.",
        "O OMSI fechou antes de pedir a posição inicial.",
    ]),
    ("No apareció la ventana de posición inicial de OMSI.", [
        "OMSI's start position window did not appear.",
        "Das Startpositionsfenster von OMSI erschien nicht.",
        "Okno pozycji początkowej OMSI nie pojawiło się.",
        "Okno počáteční pozice OMSI se neobjevilo.",
        "La fenêtre de position initiale d'OMSI n'est pas apparue.",
        "A janela de posição inicial do OMSI não apareceu.",
    ]),
    ("La ventana de posición inicial de OMSI no tiene la lista esperada.", [
        "OMSI's start position window does not have the expected list.",
        "Das Startpositionsfenster von OMSI hat nicht die erwartete Liste.",
        "Okno pozycji początkowej OMSI nie ma oczekiwanej listy.",
        "Okno počáteční pozice OMSI nemá očekávaný seznam.",
        "La fenêtre de position initiale d'OMSI n'a pas la liste attendue.",
        "A janela de posição inicial do OMSI não tem a lista esperada.",
    ]),
    ("OMSI no lista el punto \"{}\".", ["OMSI does not list the place \"{}\".", "OMSI listet den Ort \"{}\" nicht.", "OMSI nie wyświetla miejsca \"{}\".", "OMSI nezobrazuje místo \"{}\".", "OMSI ne liste pas le lieu \"{}\".", "O OMSI não lista o local \"{}\"."]),
    ("No se encontró el botón Aceptar de OMSI.", ["OMSI's Accept button was not found.", "Der OK-Button von OMSI wurde nicht gefunden.", "Nie znaleziono przycisku OK w OMSI.", "Tlačítko OK v OMSI nebylo nalezeno.", "Le bouton OK d'OMSI est introuvable.", "O botão OK do OMSI não foi encontrado."]),
    ("Abrir carpeta de copias", ["Open backups folder", "Sicherungsordner öffnen", "Otwórz folder kopii", "Otevřít složku záloh", "Ouvrir le dossier des sauvegardes", "Abrir pasta de cópias"]),
    ("Parche de 4 GB", ["4 GB patch", "4-GB-Patch", "Łatka 4 GB", "Záplata 4 GB", "Patch 4 Go", "Patch de 4 GB"]),
    ("Cambia un único bit de la cabecera de Omsi.exe para que OMSI pueda usar hasta 4 GB de memoria en vez de 2 GB. No acelera nada; evita cierres por falta de memoria. Antes se guarda una copia de Omsi.exe en la carpeta de copias. Steam puede deshacerlo al actualizar o verificar los archivos.", [
        "Changes a single bit in Omsi.exe's header so OMSI can use up to 4 GB of memory instead of 2 GB. It does not make anything faster; it prevents crashes from running out of memory. A copy of Omsi.exe is saved to the backups folder first. Steam may undo it when it updates or verifies the files.",
        "Ändert ein einzelnes Bit im Header von Omsi.exe, damit OMSI bis zu 4 GB Speicher statt 2 GB nutzen kann. Es beschleunigt nichts, verhindert aber Abstürze wegen Speichermangels. Vorher wird eine Kopie von Omsi.exe im Sicherungsordner gespeichert. Steam kann es beim Aktualisieren oder Überprüfen der Dateien rückgängig machen.",
        "Zmienia jeden bit w nagłówku Omsi.exe, aby OMSI mogło używać do 4 GB pamięci zamiast 2 GB. Niczego nie przyspiesza; zapobiega zamknięciom z powodu braku pamięci. Wcześniej kopia Omsi.exe jest zapisywana w folderze kopii. Steam może to cofnąć przy aktualizacji lub weryfikacji plików.",
        "Změní jediný bit v hlavičce Omsi.exe, aby OMSI mohlo používat až 4 GB paměti místo 2 GB. Nic nezrychlí; zabrání pádům kvůli nedostatku paměti. Předtím se kopie Omsi.exe uloží do složky záloh. Steam to může vrátit při aktualizaci nebo ověření souborů.",
        "Modifie un seul bit de l'en-tête d'Omsi.exe pour qu'OMSI puisse utiliser jusqu'à 4 Go de mémoire au lieu de 2 Go. Cela n'accélère rien ; cela évite les plantages par manque de mémoire. Une copie d'Omsi.exe est d'abord enregistrée dans le dossier des sauvegardes. Steam peut l'annuler lors d'une mise à jour ou d'une vérification des fichiers.",
        "Altera um único bit do cabeçalho do Omsi.exe para que o OMSI possa usar até 4 GB de memória em vez de 2 GB. Não acelera nada; evita falhas por falta de memória. Antes é guardada uma cópia do Omsi.exe na pasta de cópias. O Steam pode desfazê-lo ao atualizar ou verificar os ficheiros.",
    ]),
    ("Omsi.exe: parche de 4 GB activado.", ["Omsi.exe: 4 GB patch on.", "Omsi.exe: 4-GB-Patch aktiv.", "Omsi.exe: łatka 4 GB włączona.", "Omsi.exe: záplata 4 GB zapnutá.", "Omsi.exe : patch 4 Go activé.", "Omsi.exe: patch de 4 GB ativado."]),
    ("Omsi.exe: parche de 4 GB desactivado.", ["Omsi.exe: 4 GB patch off.", "Omsi.exe: 4-GB-Patch inaktiv.", "Omsi.exe: łatka 4 GB wyłączona.", "Omsi.exe: záplata 4 GB vypnutá.", "Omsi.exe : patch 4 Go désactivé.", "Omsi.exe: patch de 4 GB desativado."]),
    ("Omsi.exe: no se pudo leer.", ["Omsi.exe: could not be read.", "Omsi.exe: konnte nicht gelesen werden.", "Omsi.exe: nie można odczytać.", "Omsi.exe: nelze načíst.", "Omsi.exe : lecture impossible.", "Omsi.exe: não foi possível ler."]),
    ("¿Modificar Omsi.exe ahora? Se guarda una copia antes.", ["Modify Omsi.exe now? A copy is saved first.", "Omsi.exe jetzt ändern? Vorher wird eine Kopie gespeichert.", "Zmodyfikować Omsi.exe teraz? Wcześniej zostanie zapisana kopia.", "Upravit Omsi.exe nyní? Předtím se uloží kopie.", "Modifier Omsi.exe maintenant ? Une copie est d'abord enregistrée.", "Modificar o Omsi.exe agora? Antes é guardada uma cópia."]),
    ("Confirmar", ["Confirm", "Bestätigen", "Potwierdź", "Potvrdit", "Confirmer", "Confirmar"]),
    ("Aplicar parche de 4 GB", ["Apply 4 GB patch", "4-GB-Patch anwenden", "Zastosuj łatkę 4 GB", "Použít záplatu 4 GB", "Appliquer le patch 4 Go", "Aplicar patch de 4 GB"]),
    ("Quitar parche de 4 GB", ["Remove 4 GB patch", "4-GB-Patch entfernen", "Usuń łatkę 4 GB", "Odebrat záplatu 4 GB", "Retirer le patch 4 Go", "Remover patch de 4 GB"]),
    ("Cierra OMSI para modificar Omsi.exe.", ["Close OMSI to modify Omsi.exe.", "Schließe OMSI, um Omsi.exe zu ändern.", "Zamknij OMSI, aby zmodyfikować Omsi.exe.", "Zavři OMSI, abys mohl upravit Omsi.exe.", "Fermez OMSI pour modifier Omsi.exe.", "Feche o OMSI para modificar o Omsi.exe."]),
    ("Parche de 4 GB aplicado a Omsi.exe (copia en la carpeta de copias).", ["4 GB patch applied to Omsi.exe (copy in the backups folder).", "4-GB-Patch auf Omsi.exe angewendet (Kopie im Sicherungsordner).", "Łatka 4 GB zastosowana w Omsi.exe (kopia w folderze kopii).", "Záplata 4 GB použita na Omsi.exe (kopie ve složce záloh).", "Patch 4 Go appliqué à Omsi.exe (copie dans le dossier des sauvegardes).", "Patch de 4 GB aplicado ao Omsi.exe (cópia na pasta de cópias)."]),
    ("Parche de 4 GB quitado de Omsi.exe (copia en la carpeta de copias).", ["4 GB patch removed from Omsi.exe (copy in the backups folder).", "4-GB-Patch von Omsi.exe entfernt (Kopie im Sicherungsordner).", "Łatka 4 GB usunięta z Omsi.exe (kopia w folderze kopii).", "Záplata 4 GB odebrána z Omsi.exe (kopie ve složce záloh).", "Patch 4 Go retiré d'Omsi.exe (copie dans le dossier des sauvegardes).", "Patch de 4 GB removido do Omsi.exe (cópia na pasta de cópias)."]),
    ("No se pudo modificar Omsi.exe: {}", ["Could not modify Omsi.exe: {}", "Omsi.exe konnte nicht geändert werden: {}", "Nie można zmodyfikować Omsi.exe: {}", "Omsi.exe nelze upravit: {}", "Impossible de modifier Omsi.exe : {}", "Não foi possível modificar o Omsi.exe: {}"]),
    ("Apariencia", ["Appearance", "Darstellung", "Wygląd", "Vzhled", "Apparence", "Aparência"]),
    ("Copias de seguridad", ["Backups", "Sicherungen", "Kopie zapasowe", "Zálohy", "Sauvegardes", "Cópias de segurança"]),
    ("Actualizaciones", ["Updates", "Updates", "Aktualizacje", "Aktualizace", "Mises à jour", "Atualizações"]),
    ("Versión instalada: {}", ["Installed version: {}", "Installierte Version: {}", "Zainstalowana wersja: {}", "Nainstalovaná verze: {}", "Version installée : {}", "Versão instalada: {}"]),
    ("Buscar actualizaciones", ["Check for updates", "Nach Updates suchen", "Sprawdź aktualizacje", "Hledat aktualizace", "Rechercher des mises à jour", "Procurar atualizações"]),
    ("Buscando...", ["Checking...", "Suche...", "Szukanie...", "Hledání...", "Recherche...", "A procurar..."]),
    ("Tienes la última versión.", ["You have the latest version.", "Du hast die neueste Version.", "Masz najnowszą wersję.", "Máš nejnovější verzi.", "Vous avez la dernière version.", "Tens a versão mais recente."]),
    ("Hay una versión nueva: {}", ["A new version is available: {}", "Neue Version verfügbar: {}", "Dostępna nowa wersja: {}", "K dispozici je nová verze: {}", "Nouvelle version disponible : {}", "Há uma versão nova: {}"]),
    ("Abrir la página de descarga", ["Open the download page", "Download-Seite öffnen", "Otwórz stronę pobierania", "Otevřít stránku ke stažení", "Ouvrir la page de téléchargement", "Abrir a página de transferência"]),
    ("No se pudo comprobar (¿sin conexión o aún sin versiones publicadas?).", [
        "Could not check (no connection, or no release published yet?).",
        "Prüfung nicht möglich (keine Verbindung oder noch keine Version veröffentlicht?).",
        "Nie udało się sprawdzić (brak połączenia lub brak opublikowanych wersji?).",
        "Nelze zkontrolovat (bez připojení, nebo zatím žádné vydání?).",
        "Vérification impossible (pas de connexion ou aucune version publiée ?).",
        "Não foi possível verificar (sem ligação ou ainda sem versões publicadas?).",
    ]),
    ("Las versiones se publican en github.com/{}. El launcher solo avisa: no descarga ni cambia nada por sí solo.", [
        "Releases are published at github.com/{}. The launcher only tells you: it downloads and changes nothing by itself.",
        "Versionen erscheinen auf github.com/{}. Der Launcher weist nur darauf hin: Er lädt nichts herunter und ändert nichts selbst.",
        "Wersje są publikowane na github.com/{}. Launcher tylko informuje: sam niczego nie pobiera ani nie zmienia.",
        "Verze vycházejí na github.com/{}. Launcher jen upozorní: sám nic nestahuje ani nemění.",
        "Les versions sont publiées sur github.com/{}. Le launcher ne fait qu'avertir : il ne télécharge ni ne modifie rien seul.",
        "As versões são publicadas em github.com/{}. O launcher apenas avisa: não transfere nem altera nada sozinho.",
    ]),
    ("Se conservan las 3 últimas copias de cada archivo; al hacer una nueva se borra la más antigua.", [
        "The 3 newest backups of each file are kept; making a new one deletes the oldest.",
        "Die 3 neuesten Sicherungen jeder Datei bleiben erhalten; eine neue löscht die älteste.",
        "Zachowywane są 3 najnowsze kopie każdego pliku; nowa kopia usuwa najstarszą.",
        "Uchovávají se 3 nejnovější zálohy každého souboru; nová záloha smaže nejstarší.",
        "Les 3 sauvegardes les plus récentes de chaque fichier sont conservées ; une nouvelle supprime la plus ancienne.",
        "Mantêm-se as 3 cópias mais recentes de cada ficheiro; ao fazer uma nova apaga-se a mais antiga.",
    ]),
    ("Nombre del preset", ["Preset name", "Name der Voreinstellung", "Nazwa presetu", "Název předvolby", "Nom du préréglage", "Nome da predefinição"]),
    ("Guardar como preset", ["Save as preset", "Als Voreinstellung speichern", "Zapisz jako preset", "Uložit jako předvolbu", "Enregistrer comme préréglage", "Guardar como predefinição"]),
    ("Preset \"{}\" guardado en option_presets.", ["Preset \"{}\" saved in option_presets.", "Voreinstellung \"{}\" in option_presets gespeichert.", "Zapisano preset \"{}\" w option_presets.", "Předvolba \"{}\" uložena v option_presets.", "Préréglage \"{}\" enregistré dans option_presets.", "Predefinição \"{}\" guardada em option_presets."]),
    ("Ya existe un preset con ese nombre.", ["A preset with that name already exists.", "Eine Voreinstellung mit diesem Namen existiert bereits.", "Preset o tej nazwie już istnieje.", "Předvolba s tímto názvem již existuje.", "Un préréglage portant ce nom existe déjà.", "Já existe uma predefinição com esse nome."]),
    ("Escribe un nombre para el preset.", ["Type a name for the preset.", "Gib einen Namen für die Voreinstellung ein.", "Wpisz nazwę presetu.", "Zadejte název předvolby.", "Saisissez un nom pour le préréglage.", "Escreva um nome para a predefinição."]),
    ("No se pudo guardar el preset: {}", ["Could not save the preset: {}", "Voreinstellung konnte nicht gespeichert werden: {}", "Nie można zapisać presetu: {}", "Předvolbu nelze uložit: {}", "Impossible d'enregistrer le préréglage : {}", "Não foi possível guardar a predefinição: {}"]),
    ("Conductor visible en tu propio autobús", ["Driver visible in your own bus", "Fahrer im eigenen Bus sichtbar", "Kierowca widoczny we własnym autobusie", "Řidič viditelný ve vlastním autobusu", "Conducteur visible dans votre propre bus", "Condutor visível no seu próprio autocarro"]),
    ("Cargar/Guardar", ["Load/Save", "Laden/Speichern", "Wczytaj/Zapisz", "Načíst/Uložit", "Charger/Enregistrer", "Carregar/Guardar"]),
    ("Opciones de carga:", ["Load options:", "Optionen laden:", "Wczytaj opcje:", "Načíst možnosti:", "Charger des options :", "Carregar opções:"]),
    ("Cargar", ["Load", "Laden", "Wczytaj", "Načíst", "Charger", "Carregar"]),
    ("Guardar opciones actuales:", ["Save current options:", "Aktuelle Optionen speichern:", "Zapisz bieżące opcje:", "Uložit aktuální možnosti:", "Enregistrer les options actuelles :", "Guardar as opções atuais:"]),
    ("¡Guardar!", ["Save!", "Speichern!", "Zapisz!", "Uložit!", "Enregistrer !", "Guardar!"]),
    ("Opciones avanzadas del juego", ["Advanced game options", "Erweiterte Spieloptionen", "Zaawansowane opcje gry", "Pokročilé možnosti hry", "Options avancées du jeu", "Opções avançadas do jogo"]),
    ("Colisiones", ["Collisions", "Kollisionen", "Kolizje", "Kolize", "Collisions", "Colisões"]),
    ("Rendimiento", ["Performance", "Leistung", "Wydajność", "Výkon", "Performance", "Desempenho"]),
    ("Sistemas de partículas", ["Particle systems", "Partikelsysteme", "Systemy cząsteczek", "Částicové systémy", "Systèmes de particules", "Sistemas de partículas"]),
    ("Modo económico de reflexiones forzadas", ["Forced economy reflections mode", "Erzwungener Spiegelungs-Sparmodus", "Wymuszony tryb ekonomiczny odbić", "Vynucený úsporný režim odrazů", "Mode économique de reflets forcé", "Modo económico de reflexos forçado"]),
    ("Reducción dinámica del número de losas", ["Dynamic tile count reduction", "Dynamische Kachelreduzierung", "Dynamiczna redukcja liczby kafelków", "Dynamické snížení počtu dlaždic", "Réduction dynamique du nombre de dalles", "Redução dinâmica do número de blocos"]),
    ("Texturas...", ["Textures...", "Texturen...", "Tekstury...", "Textury...", "Textures...", "Texturas..."]),
    ("Usar canales de material...", ["Use material channels...", "Materialkanäle verwenden...", "Używaj kanałów materiału...", "Použít kanály materiálu...", "Utiliser les canaux de matériau...", "Usar canais de material..."]),
    ("Usar fecha/hora actuales", ["Use current date/time", "Aktuelles Datum/Uhrzeit verwenden", "Użyj bieżącej daty/godziny", "Použít aktuální datum/čas", "Utiliser la date/heure actuelles", "Usar data/hora atuais"]),
    ("Aceptar", ["OK", "OK", "OK", "OK", "OK", "OK"]),
    ("Año (± desplazamiento dep. del mapa)", ["Year (± map-dependent offset)", "Jahr (± kartenabhängige Verschiebung)", "Rok (± przesunięcie zależne od mapy)", "Rok (± posun závislý na mapě)", "Année (± décalage selon la carte)", "Ano (± deslocamento conforme o mapa)"]),
    ("Activo", ["Active", "Aktiv", "Aktywny", "Aktivní", "Actif", "Ativo"]),
    ("Complejidad máxima de objetos", ["Maximum object complexity", "Maximale Objektkomplexität", "Maksymalna złożoność obiektów", "Maximální složitost objektů", "Complexité maximale des objets", "Complexidade máxima dos objetos"]),
    ("Complejidad máxima del mapa", ["Maximum map complexity", "Maximale Kartenkomplexität", "Maksymalna złożoność mapy", "Maximální složitost mapy", "Complexité maximale de la carte", "Complexidade máxima do mapa"]),
    ("No programado (ej.: coches)", ["Unscheduled (e.g. cars)", "Nicht fahrplangebunden (z. B. Autos)", "Bez rozkładu (np. samochody)", "Mimo jízdní řád (např. auta)", "Non programmé (ex. : voitures)", "Não programado (ex.: carros)"]),
    ("Factor de tráfico en la carretera", ["Road traffic factor", "Verkehrsfaktor auf der Straße", "Współczynnik ruchu na drodze", "Faktor provozu na silnici", "Facteur de trafic sur la route", "Fator de tráfego na estrada"]),
    ("Coches estacionados", ["Parked cars", "Geparkte Autos", "Zaparkowane samochody", "Zaparkovaná auta", "Voitures stationnées", "Carros estacionados"]),
    ("Factor de pasajeros en estaciones", ["Passenger factor at stops", "Fahrgastfaktor an Haltestellen", "Współczynnik pasażerów na przystankach", "Faktor cestujících na zastávkách", "Facteur de passagers aux arrêts", "Fator de passageiros nas paragens"]),
    ("El valor de Recuento máx. de personas es el límite superior para todas las personas visibles.", [
        "The max. people count is the upper limit for all visible people.",
        "Die max. Personenzahl ist die Obergrenze für alle sichtbaren Personen.",
        "Maks. liczba osób jest górnym limitem dla wszystkich widocznych osób.",
        "Max. počet osob je horní hranice pro všechny viditelné osoby.",
        "Le nombre max. de personnes est la limite supérieure pour toutes les personnes visibles.",
        "A contagem máx. de pessoas é o limite superior para todas as pessoas visíveis.",
    ]),
    ("¡Ten en cuenta que no puede haber más pasajeros que el valor de Recuento máx. de personas en todas las estaciones del área visible!", [
        "Note that there cannot be more passengers than the max. people count across all stops in the visible area!",
        "Beachte: Es kann nicht mehr Fahrgäste geben als die max. Personenzahl an allen Haltestellen im sichtbaren Bereich!",
        "Pamiętaj, że pasażerów nie może być więcej niż maks. liczba osób na wszystkich przystankach w widocznym obszarze!",
        "Pozor: cestujících nemůže být víc než max. počet osob na všech zastávkách ve viditelné oblasti!",
        "Attention : il ne peut pas y avoir plus de passagers que le nombre max. de personnes sur tous les arrêts de la zone visible !",
        "Atenção: não pode haver mais passageiros do que a contagem máx. de pessoas em todas as paragens da área visível!",
    ]),
    ("Inicio directo (plugin)", ["Direct start (plugin)", "Direktstart (Plugin)", "Bezpośredni start (wtyczka)", "Přímý start (zásuvný modul)", "Démarrage direct (plugin)", "Início direto (plugin)"]),
    ("Instalar plugin", ["Install plugin", "Plugin installieren", "Zainstaluj wtyczkę", "Nainstalovat modul", "Installer le plugin", "Instalar plugin"]),
    ("Quitar plugin", ["Remove plugin", "Plugin entfernen", "Usuń wtyczkę", "Odebrat modul", "Retirer le plugin", "Remover plugin"]),
    ("Plugin instalado.", ["Plugin installed.", "Plugin installiert.", "Wtyczka zainstalowana.", "Modul nainstalován.", "Plugin installé.", "Plugin instalado."]),
    ("Plugin instalado (versión antigua: vuelve a instalarlo).", ["Plugin installed (old version: install it again).", "Plugin installiert (alte Version: erneut installieren).", "Wtyczka zainstalowana (stara wersja: zainstaluj ponownie).", "Modul nainstalován (stará verze: nainstalujte znovu).", "Plugin installé (ancienne version : réinstallez-le).", "Plugin instalado (versão antiga: instale de novo)."]),
    ("Plugin no instalado.", ["Plugin not installed.", "Plugin nicht installiert.", "Wtyczka nie jest zainstalowana.", "Modul není nainstalován.", "Plugin non installé.", "Plugin não instalado."]),
    ("Añade dos archivos nuevos a la carpeta plugins de OMSI (OMSI2Launcher.opl y OMSI2LauncherPlugin.dll) y no modifica ninguno existente. Con él, ¡Iniciar! se pulsa dentro de OMSI en vez de hacer clic con el ratón. Funciona la próxima vez que abras OMSI.", [
        "Adds two new files to OMSI's plugins folder (OMSI2Launcher.opl and OMSI2LauncherPlugin.dll) and changes no existing one. With it, Start! is pressed inside OMSI instead of clicking with the mouse. It works the next time you open OMSI.",
        "Fügt zwei neue Dateien zum Ordner plugins von OMSI hinzu (OMSI2Launcher.opl und OMSI2LauncherPlugin.dll) und ändert keine vorhandene. Damit wird Starten! in OMSI gedrückt statt mit der Maus zu klicken. Es wirkt beim nächsten Start von OMSI.",
        "Dodaje dwa nowe pliki do folderu plugins OMSI (OMSI2Launcher.opl i OMSI2LauncherPlugin.dll) i nie zmienia żadnego istniejącego. Dzięki temu Start! jest naciskany w OMSI zamiast kliknięcia myszą. Działa przy następnym uruchomieniu OMSI.",
        "Přidá dva nové soubory do složky plugins OMSI (OMSI2Launcher.opl a OMSI2LauncherPlugin.dll) a žádný existující nemění. Spustit! se pak stiskne uvnitř OMSI místo kliknutí myší. Funguje při příštím spuštění OMSI.",
        "Ajoute deux nouveaux fichiers au dossier plugins d'OMSI (OMSI2Launcher.opl et OMSI2LauncherPlugin.dll) sans modifier aucun fichier existant. Démarrer ! est alors pressé dans OMSI au lieu de cliquer avec la souris. Actif à la prochaine ouverture d'OMSI.",
        "Adiciona dois ficheiros novos à pasta plugins do OMSI (OMSI2Launcher.opl e OMSI2LauncherPlugin.dll) e não altera nenhum existente. Com ele, Iniciar! é premido dentro do OMSI em vez de clicar com o rato. Funciona da próxima vez que abrir o OMSI.",
    ]),
    ("Plugin instalado en la carpeta plugins de OMSI. Funciona la próxima vez que abras OMSI.", [
        "Plugin installed in OMSI's plugins folder. It works the next time you open OMSI.",
        "Plugin im Ordner plugins von OMSI installiert. Es wirkt beim nächsten Start von OMSI.",
        "Wtyczka zainstalowana w folderze plugins OMSI. Działa przy następnym uruchomieniu OMSI.",
        "Modul nainstalován do složky plugins OMSI. Funguje při příštím spuštění OMSI.",
        "Plugin installé dans le dossier plugins d'OMSI. Actif à la prochaine ouverture d'OMSI.",
        "Plugin instalado na pasta plugins do OMSI. Funciona da próxima vez que abrir o OMSI.",
    ]),
    ("Plugin quitado de la carpeta plugins de OMSI.", ["Plugin removed from OMSI's plugins folder.", "Plugin aus dem Ordner plugins von OMSI entfernt.", "Usunięto wtyczkę z folderu plugins OMSI.", "Modul odebrán ze složky plugins OMSI.", "Plugin retiré du dossier plugins d'OMSI.", "Plugin removido da pasta plugins do OMSI."]),
    ("No se pudo instalar el plugin: {}", ["Could not install the plugin: {}", "Plugin konnte nicht installiert werden: {}", "Nie można zainstalować wtyczki: {}", "Modul nelze nainstalovat: {}", "Impossible d'installer le plugin : {}", "Não foi possível instalar o plugin: {}"]),
    ("No se pudo quitar el plugin: {}", ["Could not remove the plugin: {}", "Plugin konnte nicht entfernt werden: {}", "Nie można usunąć wtyczki: {}", "Modul nelze odebrat: {}", "Impossible de retirer le plugin : {}", "Não foi possível remover o plugin: {}"]),
    ("El plugin no respondió: se pulsa con el ratón.", [
        "The plugin did not answer: the mouse is used instead.",
        "Das Plugin antwortete nicht: stattdessen wird die Maus benutzt.",
        "Wtyczka nie odpowiedziała: używana jest mysz.",
        "Modul neodpověděl: použije se myš.",
        "Le plugin n'a pas répondu : la souris est utilisée à la place.",
        "O plugin não respondeu: usa-se o rato.",
    ]),
    ("Mantener el launcher al frente mientras OMSI se abre", ["Keep the launcher always on top", "Launcher immer im Vordergrund halten", "Zawsze wyświetlaj launcher na wierzchu", "Udržovat launcher vždy navrchu", "Garder le launcher toujours au premier plan", "Manter o launcher sempre à frente"]),
    ("Abrir el launcher a pantalla completa (F11)", ["Open the launcher full screen (F11)", "Launcher im Vollbild öffnen (F11)", "Otwieraj launcher na pełnym ekranie (F11)", "Otevřít launcher na celou obrazovku (F11)", "Ouvrir le launcher en plein écran (F11)", "Abrir o launcher em ecrã inteiro (F11)"]),
    ("Se aplicará al reiniciar el launcher (F11 lo cambia ahora).", ["This will be applied when the launcher is restarted (F11 changes it now).", "Wird beim nächsten Start des Launchers übernommen (F11 ändert es sofort).", "Zostanie zastosowane po ponownym uruchomieniu launchera (F11 zmienia to od razu).", "Použije se po restartu launcheru (F11 to změní hned).", "Sera appliqué au redémarrage du launcher (F11 le change tout de suite).", "Será aplicado ao reiniciar o launcher (F11 muda já)."]),
    ("Cargando OMSI 2...", ["Loading OMSI 2...", "OMSI 2 wird geladen...", "Wczytywanie OMSI 2...", "Načítám OMSI 2...", "Chargement d'OMSI 2...", "A carregar o OMSI 2..."]),
    ("Esto puede tardar un par de minutos.", ["This can take a couple of minutes.", "Das kann ein paar Minuten dauern.", "To może potrwać kilka minut.", "Může to trvat několik minut.", "Cela peut prendre quelques minutes.", "Isto pode demorar alguns minutos."]),
    ("Tiempo transcurrido: {}", ["Elapsed time: {}", "Vergangene Zeit: {}", "Upłynęło: {}", "Uplynulý čas: {}", "Temps écoulé : {}", "Tempo decorrido: {}"]),
    ("Ocultar el launcher", ["Hide the launcher", "Launcher ausblenden", "Ukryj launcher", "Skrýt launcher", "Masquer le launcher", "Ocultar o launcher"]),
    ("Mapa:", ["Map:", "Karte:", "Mapa:", "Mapa:", "Carte :", "Mapa:"]),
    ("Tema", ["Theme", "Design", "Motyw", "Motiv", "Thème", "Tema"]),
    ("Original (naranja de OMSI 2)", ["Original (OMSI 2 orange)", "Original (OMSI-2-Orange)", "Oryginalny (pomarańczowy OMSI 2)", "Původní (oranžová OMSI 2)", "Original (orange d'OMSI 2)", "Original (laranja do OMSI 2)"]),
    ("Oscuro", ["Dark", "Dunkel", "Ciemny", "Tmavý", "Sombre", "Escuro"]),
    ("Claro", ["Light", "Hell", "Jasny", "Světlý", "Clair", "Claro"]),
    ("Abrir OMSI en modo ventana", [
        "Open OMSI in windowed mode",
        "OMSI im Fenstermodus starten",
        "Uruchom OMSI w oknie",
        "Spustit OMSI v okně",
        "Ouvrir OMSI en mode fenêtré",
        "Abrir o OMSI em modo janela",
    ]),
    ("Carpeta: {}", ["Folder: {}", "Ordner: {}", "Katalog: {}", "Složka: {}", "Dossier : {}", "Pasta: {}"]),
    ("no encontrada", ["not found", "nicht gefunden", "nie znaleziono", "nenalezena", "introuvable", "não encontrada"]),
    ("Otra carpeta:", ["Other folder:", "Anderer Ordner:", "Inny folder:", "Jiná složka:", "Autre dossier :", "Outra pasta:"]),
    ("(la carpeta nueva se usa al reiniciar el launcher)", [
        "(the new folder is used after restarting the launcher)",
        "(der neue Ordner gilt nach einem Neustart des Launchers)",
        "(nowy folder zostanie użyty po ponownym uruchomieniu launchera)",
        "(nová složka se použije po restartu launcheru)",
        "(le nouveau dossier est utilisé après le redémarrage du launcher)",
        "(a nova pasta é usada depois de reiniciar o launcher)",
    ]),
    // ---- drivers
    ("Nombre:", ["Name:", "Name:", "Nazwa:", "Jméno:", "Nom :", "Nome:"]),
    ("Sexo:", ["Sex:", "Geschlecht:", "Płeć:", "Pohlaví:", "Sexe :", "Sexo:"]),
    ("Hombre", ["Male", "Männlich", "Mężczyzna", "Muž", "Homme", "Homem"]),
    ("Mujer", ["Female", "Weiblich", "Kobieta", "Žena", "Femme", "Mulher"]),
    ("Nacimiento:", ["Birth date:", "Geburtsdatum:", "Data urodzenia:", "Datum narození:", "Naissance :", "Nascimento:"]),
    ("(día / mes / año)", ["(day / month / year)", "(Tag / Monat / Jahr)", "(dzień / miesiąc / rok)", "(den / měsíc / rok)", "(jour / mois / année)", "(dia / mês / ano)"]),
    ("Guardar", ["Save", "Speichern", "Zapisz", "Uložit", "Enregistrer", "Guardar"]),
    ("Cancelar", ["Cancel", "Abbrechen", "Anuluj", "Zrušit", "Annuler", "Cancelar"]),
    ("Abrir archivo de personal", ["Open personnel file…", "Fahrerakte öffnen", "Otwórz akta personelu", "Otevřít osobní spis", "Ouvrir le dossier du personnel", "Abrir ficha do pessoal"]),
    ("Expediente del personal", ["Personnel File", "Personalakte", "Akta personelu", "Osobní spis", "Dossier du personnel", "Ficha do pessoal"]),
    ("Eligiendo la situación guardada...", ["Choosing the saved situation...", "Gespeicherte Situation wird gewählt...", "Wybieram zapisaną sytuację...", "Vybírám uloženou situaci...", "Choix de la situation enregistrée...", "A escolher a situação guardada..."]),
    ("OMSI no lista la situación \"{}\".", ["OMSI does not list the situation \"{}\".", "OMSI listet die Situation \"{}\" nicht.", "OMSI nie zawiera sytuacji \"{}\".", "OMSI nezná situaci \"{}\".", "OMSI ne liste pas la situation \"{}\".", "O OMSI não lista a situação \"{}\"."]),
    ("No apareció la lista de situaciones de OMSI.", [
        "OMSI's situation list did not appear.",
        "Die Situationsliste von OMSI erschien nicht.",
        "Lista sytuacji OMSI nie pojawiła się.",
        "Seznam situací OMSI se nezobrazil.",
        "La liste des situations d'OMSI n'est pas apparue.",
        "A lista de situações do OMSI não apareceu.",
    ]),
    ("Solo las situaciones del mapa seleccionado", ["Only situations of the selected map", "Nur Situationen der gewählten Karte", "Tylko sytuacje wybranej mapy", "Pouze situace vybrané mapy", "Seulement les situations de la carte sélectionnée", "Só situações do mapa selecionado"]),
    ("Información sobre la situación seleccionada:", [
        "Information about the selected situation:",
        "Informationen zur gewählten Situation:",
        "Informacje o wybranej sytuacji:",
        "Informace o vybrané situaci:",
        "Informations sur la situation sélectionnée :",
        "Informações sobre a situação selecionada:",
    ]),
    ("¡No hay situación seleccionada!", ["No situation selected!", "Keine Situation gewählt!", "Nie wybrano sytuacji!", "Není vybrána žádná situace!", "Aucune situation sélectionnée !", "Nenhuma situação selecionada!"]),
    ("No hay situaciones guardadas para este mapa.", [
        "There are no saved situations for this map.",
        "Für diese Karte gibt es keine gespeicherten Situationen.",
        "Brak zapisanych sytuacji dla tej mapy.",
        "Pro tuto mapu nejsou uložené žádné situace.",
        "Il n'y a pas de situation enregistrée pour cette carte.",
        "Não há situações guardadas para este mapa.",
    ]),
    ("Eliminar", ["Delete", "Löschen", "Usuń", "Smazat", "Supprimer", "Eliminar"]),
    ("¿Eliminar al conductor {}? Se guardará antes una copia de seguridad.", [
        "Delete driver {}? A backup copy is saved first.",
        "Fahrer {} löschen? Zuvor wird eine Sicherung gespeichert.",
        "Usunąć kierowcę {}? Najpierw zostanie zapisana kopia zapasowa.",
        "Smazat řidiče {}? Nejprve se uloží záloha.",
        "Supprimer le conducteur {} ? Une sauvegarde est faite avant.",
        "Eliminar o condutor {}? Antes é guardada uma cópia de segurança.",
    ]),
    ("Conductor eliminado (copia de seguridad en %APPDATA%\\OMSI2-Launcher\\backups).", [
        "Driver deleted (backup in %APPDATA%\\OMSI2-Launcher\\backups).",
        "Fahrer gelöscht (Sicherung in %APPDATA%\\OMSI2-Launcher\\backups).",
        "Usunięto kierowcę (kopia zapasowa w %APPDATA%\\OMSI2-Launcher\\backups).",
        "Řidič smazán (záloha v %APPDATA%\\OMSI2-Launcher\\backups).",
        "Conducteur supprimé (sauvegarde dans %APPDATA%\\OMSI2-Launcher\\backups).",
        "Condutor eliminado (cópia de segurança em %APPDATA%\\OMSI2-Launcher\\backups).",
    ]),
    ("Conductor guardado (copia de seguridad en %APPDATA%\\OMSI2-Launcher\\backups).", [
        "Driver saved (backup in %APPDATA%\\OMSI2-Launcher\\backups).",
        "Fahrer gespeichert (Sicherung in %APPDATA%\\OMSI2-Launcher\\backups).",
        "Zapisano kierowcę (kopia zapasowa w %APPDATA%\\OMSI2-Launcher\\backups).",
        "Řidič uložen (záloha v %APPDATA%\\OMSI2-Launcher\\backups).",
        "Conducteur enregistré (sauvegarde dans %APPDATA%\\OMSI2-Launcher\\backups).",
        "Condutor guardado (cópia de segurança em %APPDATA%\\OMSI2-Launcher\\backups).",
    ]),
    // ---- tutorials
    ("Tutorial I", ["Tutorial I", "Tutorial I", "Samouczek I", "Tutoriál I", "Tutoriel I", "Tutorial I"]),
    ("Tutorial II", ["Tutorial II", "Tutorial II", "Samouczek II", "Tutoriál II", "Tutoriel II", "Tutorial II"]),
    ("Tutorial III", ["Tutorial III", "Tutorial III", "Samouczek III", "Tutoriál III", "Tutoriel III", "Tutorial III"]),
    ("Tutorial IV", ["Tutorial IV", "Tutorial IV", "Samouczek IV", "Tutoriál IV", "Tutoriel IV", "Tutorial IV"]),
    ("Elige un tutorial", ["Choose a tutorial", "Wähle ein Tutorial", "Wybierz samouczek", "Vyberte tutoriál", "Choisissez un tutoriel", "Escolha um tutorial"]),
    ("Iniciar tutorial", ["Start tutorial", "Tutorial starten", "Rozpocznij samouczek", "Spustit tutoriál", "Lancer le tutoriel", "Iniciar tutorial"]),
    // ---- launching messages
    ("Abriendo OMSI 2...", ["Opening OMSI 2...", "OMSI 2 wird geöffnet...", "Otwieranie OMSI 2...", "Otevírám OMSI 2...", "Ouverture d'OMSI 2...", "A abrir o OMSI 2..."]),
    ("OMSI 2 ya está abierto, esperando su pantalla de inicio...", [
        "OMSI 2 is already open, waiting for its start screen...",
        "OMSI 2 ist bereits geöffnet, warte auf den Startbildschirm...",
        "OMSI 2 jest już otwarte, czekam na ekran startowy...",
        "OMSI 2 už je spuštěno, čekám na úvodní obrazovku...",
        "OMSI 2 est déjà ouvert, attente de son écran de démarrage...",
        "O OMSI 2 já está aberto, a aguardar o ecrã inicial...",
    ]),
    ("OMSI no llegó a su pantalla de inicio (10 min).", [
        "OMSI did not reach its start screen (10 min).",
        "OMSI hat den Startbildschirm nicht erreicht (10 Min.).",
        "OMSI nie dotarło do ekranu startowego (10 min).",
        "OMSI se nedostalo na úvodní obrazovku (10 min).",
        "OMSI n'a pas atteint son écran de démarrage (10 min).",
        "O OMSI não chegou ao ecrã inicial (10 min).",
    ]),
    ("Pantalla de inicio detectada, eligiendo mapa...", [
        "Start screen detected, choosing the map...",
        "Startbildschirm erkannt, Karte wird gewählt...",
        "Wykryto ekran startowy, wybieram mapę...",
        "Úvodní obrazovka nalezena, vybírám mapu...",
        "Écran de démarrage détecté, choix de la carte...",
        "Ecrã inicial detetado, a escolher o mapa...",
    ]),
    ("Pulsando ¡Iniciar!...", ["Pressing Start...", "Starten wird gedrückt...", "Naciskam Start...", "Mačkám Spustit...", "Appui sur Démarrer...", "A premir Iniciar..."]),
    ("Cargando el mapa en OMSI 2...", ["Loading the map in OMSI 2...", "Karte wird in OMSI 2 geladen...", "Wczytywanie mapy w OMSI 2...", "Načítám mapu v OMSI 2...", "Chargement de la carte dans OMSI 2...", "A carregar o mapa no OMSI 2..."]),
    ("Eligiendo fecha y hora...", ["Choosing date and time...", "Datum und Uhrzeit werden gewählt...", "Wybieram datę i godzinę...", "Vybírám datum a čas...", "Choix de la date et de l'heure...", "A escolher data e hora..."]),
    ("No se abrió el diálogo de fecha y hora de OMSI.", [
        "OMSI's date and time dialog did not open.",
        "Der Datums- und Uhrzeitdialog von OMSI wurde nicht geöffnet.",
        "Okno daty i godziny OMSI nie otworzyło się.",
        "Dialog data a času v OMSI se neotevřel.",
        "La boîte de dialogue de date et d'heure d'OMSI ne s'est pas ouverte.",
        "A caixa de data e hora do OMSI não abriu.",
    ]),
    ("El diálogo de fecha y hora de OMSI no tiene el aspecto esperado.", [
        "OMSI's date and time dialog does not look as expected.",
        "Der Datums- und Uhrzeitdialog von OMSI sieht nicht wie erwartet aus.",
        "Okno daty i godziny OMSI nie wygląda tak, jak oczekiwano.",
        "Dialog data a času v OMSI nevypadá podle očekávání.",
        "La boîte de dialogue de date et d'heure d'OMSI n'a pas l'aspect attendu.",
        "A caixa de data e hora do OMSI não tem o aspeto esperado.",
    ]),
    ("Abriendo el menú de tutoriales...", ["Opening the tutorial menu...", "Tutorial-Menü wird geöffnet...", "Otwieranie menu samouczków...", "Otevírám menu tutoriálů...", "Ouverture du menu des tutoriels...", "A abrir o menu de tutoriais..."]),
    ("No se abrió el menú de tutoriales de OMSI.", [
        "OMSI's tutorial menu did not open.",
        "Das Tutorial-Menü von OMSI wurde nicht geöffnet.",
        "Menu samouczków OMSI nie otworzyło się.",
        "Menu tutoriálů v OMSI se neotevřelo.",
        "Le menu des tutoriels d'OMSI ne s'est pas ouvert.",
        "O menu de tutoriais do OMSI não abriu.",
    ]),
    ("Cargando el tutorial en OMSI 2...", ["Loading the tutorial in OMSI 2...", "Tutorial wird in OMSI 2 geladen...", "Wczytywanie samouczka w OMSI 2...", "Načítám tutoriál v OMSI 2...", "Chargement du tutoriel dans OMSI 2...", "A carregar o tutorial no OMSI 2..."]),
    ("No se pudo abrir Omsi.exe: {}", ["Could not open Omsi.exe: {}", "Omsi.exe konnte nicht geöffnet werden: {}", "Nie można otworzyć Omsi.exe: {}", "Nelze otevřít Omsi.exe: {}", "Impossible d'ouvrir Omsi.exe : {}", "Não foi possível abrir o Omsi.exe: {}"]),
    ("OMSI no lista el mapa \"{}\".", ["OMSI does not list the map \"{}\".", "OMSI listet die Karte \"{}\" nicht.", "OMSI nie zawiera mapy \"{}\".", "OMSI nezná mapu \"{}\".", "OMSI ne liste pas la carte \"{}\".", "O OMSI não lista o mapa \"{}\"."]),
    // ---- errors writing files
    ("No se pudo crear la carpeta de copias: {}", ["Could not create the backup folder: {}", "Sicherungsordner konnte nicht erstellt werden: {}", "Nie można utworzyć folderu kopii: {}", "Nelze vytvořit složku záloh: {}", "Impossible de créer le dossier de sauvegarde : {}", "Não foi possível criar a pasta de cópias: {}"]),
    ("No se pudo hacer la copia de seguridad: {}", ["Could not make the backup: {}", "Sicherung fehlgeschlagen: {}", "Nie można wykonać kopii zapasowej: {}", "Zálohu se nepodařilo vytvořit: {}", "Impossible de faire la sauvegarde : {}", "Não foi possível fazer a cópia de segurança: {}"]),
    ("No se pudo restaurar: {}", ["Could not restore: {}", "Wiederherstellung fehlgeschlagen: {}", "Nie można przywrócić: {}", "Obnovení se nezdařilo: {}", "Impossible de restaurer : {}", "Não foi possível restaurar: {}"]),
    ("No se pudo guardar options.cfg: {}", ["Could not save options.cfg: {}", "options.cfg konnte nicht gespeichert werden: {}", "Nie można zapisać options.cfg: {}", "Nelze uložit options.cfg: {}", "Impossible d'enregistrer options.cfg : {}", "Não foi possível guardar o options.cfg: {}"]),
    ("No se pudo escribir el conductor: {}", ["Could not write the driver: {}", "Fahrer konnte nicht geschrieben werden: {}", "Nie można zapisać kierowcy: {}", "Řidiče se nepodařilo zapsat: {}", "Impossible d'écrire le conducteur : {}", "Não foi possível escrever o condutor: {}"]),
    ("No se pudo quitar el archivo anterior: {}", ["Could not remove the previous file: {}", "Die alte Datei konnte nicht entfernt werden: {}", "Nie można usunąć poprzedniego pliku: {}", "Předchozí soubor nelze odstranit: {}", "Impossible de supprimer l'ancien fichier : {}", "Não foi possível remover o ficheiro anterior: {}"]),
    ("No se pudo crear la carpeta de ajustes: {}", ["Could not create the settings folder: {}", "Einstellungsordner konnte nicht erstellt werden: {}", "Nie można utworzyć folderu ustawień: {}", "Nelze vytvořit složku nastavení: {}", "Impossible de créer le dossier des réglages : {}", "Não foi possível criar a pasta de definições: {}"]),
    ("No se pudieron guardar los ajustes: {}", ["Could not save the settings: {}", "Einstellungen konnten nicht gespeichert werden: {}", "Nie można zapisać ustawień: {}", "Nastavení se nepodařilo uložit: {}", "Impossible d'enregistrer les réglages : {}", "Não foi possível guardar as definições: {}"]),
    ("Nombre no válido (no puede estar vacío ni contener \\ / : * ? \" < > | [ ]).", [
        "Invalid name (it cannot be empty or contain \\ / : * ? \" < > | [ ]).",
        "Ungültiger Name (darf nicht leer sein oder \\ / : * ? \" < > | [ ] enthalten).",
        "Nieprawidłowa nazwa (nie może być pusta ani zawierać \\ / : * ? \" < > | [ ]).",
        "Neplatné jméno (nesmí být prázdné ani obsahovat \\ / : * ? \" < > | [ ]).",
        "Nom invalide (il ne peut pas être vide ni contenir \\ / : * ? \" < > | [ ]).",
        "Nome inválido (não pode estar vazio nem conter \\ / : * ? \" < > | [ ]).",
    ]),
    ("La fecha de nacimiento debe ser aaaammdd (por ejemplo 19950606).", [
        "The birth date must be yyyymmdd (for example 19950606).",
        "Das Geburtsdatum muss jjjjmmtt sein (zum Beispiel 19950606).",
        "Data urodzenia musi mieć format rrrrmmdd (na przykład 19950606).",
        "Datum narození musí být rrrrmmdd (například 19950606).",
        "La date de naissance doit être aaaammjj (par exemple 19950606).",
        "A data de nascimento deve ser aaaammdd (por exemplo 19950606).",
    ]),
    ("Ya existe un conductor con ese nombre.", ["A driver with that name already exists.", "Ein Fahrer mit diesem Namen existiert bereits.", "Kierowca o tej nazwie już istnieje.", "Řidič s tímto jménem již existuje.", "Un conducteur portant ce nom existe déjà.", "Já existe um condutor com esse nome."]),
    ("No se pudo leer el conductor.", ["Could not read the driver.", "Fahrer konnte nicht gelesen werden.", "Nie można odczytać kierowcy.", "Řidiče nelze přečíst.", "Impossible de lire le conducteur.", "Não foi possível ler o condutor."]),
    ("Archivo de conductor no válido.", ["Invalid driver file.", "Ungültige Fahrerdatei.", "Nieprawidłowy plik kierowcy.", "Neplatný soubor řidiče.", "Fichier de conducteur invalide.", "Ficheiro de condutor inválido."]),
    // ---- OMSI options window
    ("Opciones del juego", ["Game options", "Spieloptionen", "Opcje gry", "Možnosti hry", "Options du jeu", "Opções do jogo"]),
    ("No se pudo leer options.cfg.", ["Could not read options.cfg.", "options.cfg konnte nicht gelesen werden.", "Nie można odczytać options.cfg.", "Soubor options.cfg nelze přečíst.", "Impossible de lire options.cfg.", "Não foi possível ler o options.cfg."]),
    ("Preset de OMSI:", ["OMSI preset:", "OMSI-Voreinstellung:", "Preset OMSI:", "Předvolba OMSI:", "Préréglage OMSI :", "Predefinição do OMSI:"]),
    ("Cargar preset", ["Load preset", "Voreinstellung laden", "Wczytaj preset", "Načíst předvolbu", "Charger le préréglage", "Carregar predefinição"]),
    ("Preset \"{}\" cargado (sin guardar: pulsa Guardar en Opciones o ¡Iniciar!).", [
        "Preset \"{}\" loaded (not saved: press Save in Options or Start!).",
        "Voreinstellung \"{}\" geladen (nicht gespeichert: Speichern in den Optionen oder Starten drücken).",
        "Wczytano preset \"{}\" (niezapisany: naciśnij Zapisz w Opcjach lub Start!).",
        "Předvolba \"{}\" načtena (neuloženo: stiskněte Uložit v Možnostech nebo Spustit!).",
        "Préréglage \"{}\" chargé (non enregistré : appuyez sur Enregistrer dans Options ou sur Démarrer !).",
        "Predefinição \"{}\" carregada (não guardada: prima Guardar nas Opções ou Iniciar!).",
    ]),
    ("Un preset cambia gráficos, sonido y tráfico; no cambia mapa, conductor, idioma ni fecha.", [
        "A preset changes graphics, sound and traffic; it does not change map, driver, language or date.",
        "Eine Voreinstellung ändert Grafik, Ton und Verkehr, nicht aber Karte, Fahrer, Sprache oder Datum.",
        "Preset zmienia grafikę, dźwięk i ruch; nie zmienia mapy, kierowcy, języka ani daty.",
        "Předvolba mění grafiku, zvuk a provoz; nemění mapu, řidiče, jazyk ani datum.",
        "Un préréglage modifie les graphismes, le son et le trafic ; il ne change ni la carte, ni le conducteur, ni la langue, ni la date.",
        "Uma predefinição altera gráficos, som e tráfego; não altera mapa, condutor, idioma nem data.",
    ]),
    ("OMSI 2 está abierto: ciérralo para poder guardar cambios.", [
        "OMSI 2 is open: close it to be able to save changes.",
        "OMSI 2 ist geöffnet: Schließe es, um Änderungen zu speichern.",
        "OMSI 2 jest otwarte: zamknij je, aby zapisać zmiany.",
        "OMSI 2 je spuštěno: zavřete ho, abyste mohli uložit změny.",
        "OMSI 2 est ouvert : fermez-le pour pouvoir enregistrer les modifications.",
        "O OMSI 2 está aberto: feche-o para poder guardar alterações.",
    ]),
    ("Opciones guardadas (copia de seguridad en %APPDATA%\\OMSI2-Launcher\\backups).", [
        "Options saved (backup in %APPDATA%\\OMSI2-Launcher\\backups).",
        "Optionen gespeichert (Sicherung in %APPDATA%\\OMSI2-Launcher\\backups).",
        "Zapisano opcje (kopia zapasowa w %APPDATA%\\OMSI2-Launcher\\backups).",
        "Možnosti uloženy (záloha v %APPDATA%\\OMSI2-Launcher\\backups).",
        "Options enregistrées (sauvegarde dans %APPDATA%\\OMSI2-Launcher\\backups).",
        "Opções guardadas (cópia de segurança em %APPDATA%\\OMSI2-Launcher\\backups).",
    ]),
    ("Restaurar última copia", ["Restore last backup", "Letzte Sicherung wiederherstellen", "Przywróć ostatnią kopię", "Obnovit poslední zálohu", "Restaurer la dernière sauvegarde", "Restaurar a última cópia"]),
    ("options.cfg restaurado desde la última copia.", [
        "options.cfg restored from the last backup.",
        "options.cfg aus der letzten Sicherung wiederhergestellt.",
        "Przywrócono options.cfg z ostatniej kopii.",
        "options.cfg obnoven z poslední zálohy.",
        "options.cfg restauré depuis la dernière sauvegarde.",
        "options.cfg restaurado a partir da última cópia.",
    ]),
    ("Hay cambios sin guardar (se guardan al pulsar ¡Iniciar!).", [
        "There are unsaved changes (saved when you press Start!).",
        "Es gibt ungespeicherte Änderungen (werden beim Drücken von Starten gespeichert).",
        "Są niezapisane zmiany (zapiszą się po naciśnięciu Start!).",
        "Jsou neuložené změny (uloží se po stisknutí Spustit!).",
        "Des modifications ne sont pas enregistrées (elles le seront en appuyant sur Démarrer !).",
        "Há alterações por guardar (guardam-se ao premir Iniciar!).",
    ]),
    ("Buscar:", ["Search:", "Suchen:", "Szukaj:", "Hledat:", "Rechercher :", "Pesquisar:"]),
    ("Sin resultados", ["No results", "Keine Ergebnisse", "Brak wyników", "Žádné výsledky", "Aucun résultat", "Sem resultados"]),
    ("Escribe para buscar", ["Type to search", "Tippen zum Suchen", "Pisz, aby wyszukać", "Pište pro hledání", "Tapez pour rechercher", "Escreva para pesquisar"]),
    ("General", ["General", "Allgemein", "Ogólne", "Obecné", "Général", "Geral"]),
    ("Avanzado", ["Advanced", "Erweitert", "Zaawansowane", "Pokročilé", "Avancé", "Avançado"]),
    ("Gráficos", ["Graphics", "Grafik", "Grafika", "Grafika", "Graphismes", "Gráficos"]),
    ("Gráficos (avanzado)", ["Graphics (advanced)", "Grafik (erweitert)", "Grafika (zaawansowana)", "Grafika (pokročilá)", "Graphismes (avancé)", "Gráficos (avançado)"]),
    ("Sonido", ["Sound", "Ton", "Dźwięk", "Zvuk", "Son", "Som"]),
    ("Tráfico IA", ["AI traffic", "KI-Verkehr", "Ruch AI", "Provoz AI", "Trafic IA", "Tráfego IA"]),
    ("Ninguno", ["None", "Keine", "Brak", "Žádné", "Aucun", "Nenhum"]),
    ("Fácil", ["Easy", "Einfach", "Łatwe", "Snadné", "Facile", "Fácil"]),
    ("Económico", ["Economy", "Sparsam", "Ekonomiczne", "Úsporné", "Économique", "Económico"]),
    ("Completo", ["Full", "Voll", "Pełne", "Plné", "Complet", "Completo"]),
    ("Infinito", ["Infinite", "Unendlich", "Nieskończony", "Nekonečný", "Infini", "Infinito"]),
    ("Mantenimiento muy malo", ["Very poor maintenance", "Sehr schlechte Wartung", "Bardzo zła konserwacja", "Velmi špatná údržba", "Entretien très mauvais", "Manutenção muito má"]),
    ("Mantenimiento malo", ["Poor maintenance", "Schlechte Wartung", "Zła konserwacja", "Špatná údržba", "Mauvais entretien", "Manutenção má"]),
    ("Mantenimiento normal", ["Normal maintenance", "Normale Wartung", "Normalna konserwacja", "Normální údržba", "Entretien normal", "Manutenção normal"]),
    ("Buen mantenimiento", ["Good maintenance", "Gute Wartung", "Dobra konserwacja", "Dobrá údržba", "Bon entretien", "Boa manutenção"]),
    ("Idioma", ["Language", "Sprache", "Język", "Jazyk", "Langue", "Idioma"]),
    ("Venta de billetes", ["Ticket sales", "Fahrkartenverkauf", "Sprzedaż biletów", "Prodej jízdenek", "Vente de billets", "Venda de bilhetes"]),
    ("Vincular emisora de radio", ["Link radio station", "Radiosender verknüpfen", "Powiąż stację radiową", "Propojit rozhlasovou stanici", "Lier une station de radio", "Associar estação de rádio"]),
    ("Estado de mantenimiento", ["Maintenance condition", "Wartungszustand", "Stan konserwacji", "Stav údržby", "État d'entretien", "Estado de manutenção"]),
    ("La cabeza del conductor se mueve", ["The driver's head moves", "Der Kopf des Fahrers bewegt sich", "Głowa kierowcy się porusza", "Hlava řidiče se pohybuje", "La tête du conducteur bouge", "A cabeça do condutor mexe-se"]),
    ("Transiciones suaves de vista en la vista del conductor", [
        "Smooth view transitions in the driver's view",
        "Weiche Ansichtsübergänge in der Fahreransicht",
        "Płynne przejścia widoku w widoku kierowcy",
        "Plynulé přechody pohledu v pohledu řidiče",
        "Transitions de vue fluides dans la vue du conducteur",
        "Transições suaves de vista na vista do condutor",
    ]),
    ("Vista alternativa (zoom = [Shift] + botón derecho, desplazamiento = botón derecho)", [
        "Alternative view (zoom = [Shift] + right button, pan = right button)",
        "Alternative Ansicht (Zoom = [Umschalt] + rechte Taste, Verschieben = rechte Taste)",
        "Widok alternatywny (zoom = [Shift] + prawy przycisk, przesuwanie = prawy przycisk)",
        "Alternativní pohled (zoom = [Shift] + pravé tlačítko, posun = pravé tlačítko)",
        "Vue alternative (zoom = [Maj] + bouton droit, déplacement = bouton droit)",
        "Vista alternativa (zoom = [Shift] + botão direito, deslocamento = botão direito)",
    ]),
    ("Centro automático del volante (solo control por teclado)", [
        "Automatic steering centre (keyboard control only)",
        "Automatische Lenkungszentrierung (nur Tastatursteuerung)",
        "Automatyczne centrowanie kierownicy (tylko sterowanie klawiaturą)",
        "Automatické vystředění volantu (pouze ovládání klávesnicí)",
        "Centrage automatique du volant (commande au clavier uniquement)",
        "Centragem automática do volante (só controlo por teclado)",
    ]),
    ("Embrague automático", ["Automatic clutch", "Automatische Kupplung", "Automatyczne sprzęgło", "Automatická spojka", "Embrayage automatique", "Embraiagem automática"]),
    ("Velocidad dinámica del volante", ["Dynamic steering speed", "Dynamische Lenkgeschwindigkeit", "Dynamiczna prędkość kierownicy", "Dynamická rychlost volantu", "Vitesse de volant dynamique", "Velocidade dinâmica do volante"]),
    ("Colisiones con otros vehículos y paisaje", [
        "Collisions with other vehicles and scenery",
        "Kollisionen mit anderen Fahrzeugen und Landschaft",
        "Kolizje z innymi pojazdami i otoczeniem",
        "Kolize s jinými vozidly a krajinou",
        "Collisions avec d'autres véhicules et le décor",
        "Colisões com outros veículos e cenário",
    ]),
    ("No mostrar análisis automático del horario", [
        "Do not show automatic timetable analysis",
        "Automatische Fahrplananalyse nicht anzeigen",
        "Nie pokazuj automatycznej analizy rozkładu",
        "Nezobrazovat automatickou analýzu jízdního řádu",
        "Ne pas afficher l'analyse automatique de l'horaire",
        "Não mostrar a análise automática do horário",
    ]),
    ("Mostrar ayuda para venta de billetes y diálogo con pasajeros", [
        "Show help for ticket sales and passenger dialogue",
        "Hilfe für Fahrkartenverkauf und Fahrgastdialog anzeigen",
        "Pokaż pomoc przy sprzedaży biletów i dialogu z pasażerami",
        "Zobrazit nápovědu k prodeji jízdenek a dialogu s cestujícími",
        "Afficher l'aide pour la vente de billets et le dialogue avec les passagers",
        "Mostrar ajuda para venda de bilhetes e diálogo com passageiros",
    ]),
    ("Guardado automático", ["Automatic saving", "Automatisches Speichern", "Automatyczny zapis", "Automatické ukládání", "Sauvegarde automatique", "Gravação automática"]),
    ("Mostrar mensajes de error", ["Show error messages", "Fehlermeldungen anzeigen", "Pokaż komunikaty o błędach", "Zobrazit chybová hlášení", "Afficher les messages d'erreur", "Mostrar mensagens de erro"]),
    ("Colisiones: entre el vehículo del usuario y el terreno", [
        "Collisions: between the user's vehicle and the terrain",
        "Kollisionen: zwischen Fahrzeug des Nutzers und Gelände",
        "Kolizje: między pojazdem użytkownika a terenem",
        "Kolize: mezi vozidlem uživatele a terénem",
        "Collisions : entre le véhicule de l'utilisateur et le terrain",
        "Colisões: entre o veículo do utilizador e o terreno",
    ]),
    ("Colisiones: entre vehículos del usuario y peatones", [
        "Collisions: between the user's vehicles and pedestrians",
        "Kollisionen: zwischen Fahrzeugen des Nutzers und Fußgängern",
        "Kolizje: między pojazdami użytkownika a pieszymi",
        "Kolize: mezi vozidly uživatele a chodci",
        "Collisions : entre les véhicules de l'utilisateur et les piétons",
        "Colisões: entre veículos do utilizador e peões",
    ]),
    ("Colisiones: entre vehículos IA", ["Collisions: between AI vehicles", "Kollisionen: zwischen KI-Fahrzeugen", "Kolizje: między pojazdami AI", "Kolize: mezi vozidly AI", "Collisions : entre véhicules IA", "Colisões: entre veículos IA"]),
    ("Rendimiento: cálculo multihilo", ["Performance: multithreaded calculation", "Leistung: Multithread-Berechnung", "Wydajność: obliczenia wielowątkowe", "Výkon: vícevláknové výpočty", "Performance : calcul multithread", "Desempenho: cálculo multitarefa"]),
    ("Rendimiento: carga de texturas multihilo", ["Performance: multithreaded texture loading", "Leistung: Multithread-Texturladen", "Wydajność: wielowątkowe wczytywanie tekstur", "Výkon: vícevláknové načítání textur", "Performance : chargement des textures multithread", "Desempenho: carregamento de texturas multitarefa"]),
    ("Cargar todo el mapa al inicio", ["Load the whole map at start", "Ganze Karte beim Start laden", "Wczytaj całą mapę na początku", "Načíst celou mapu na začátku", "Charger toute la carte au démarrage", "Carregar todo o mapa no início"]),
    ("Framerate objetivo", ["Target framerate", "Ziel-Bildrate", "Docelowa liczba klatek", "Cílová snímková frekvence", "Fréquence d'images cible", "Taxa de fotogramas alvo"]),
    ("Cantidad de baldosas vecinas", ["Number of neighbouring tiles", "Anzahl benachbarter Kacheln", "Liczba sąsiednich kafelków", "Počet sousedních dlaždic", "Nombre de dalles voisines", "Número de blocos vizinhos"]),
    ("Distancia máxima de visibilidad de objetos", ["Maximum object visibility distance", "Maximale Sichtweite von Objekten", "Maksymalna odległość widoczności obiektów", "Maximální vzdálenost viditelnosti objektů", "Distance maximale de visibilité des objets", "Distância máxima de visibilidade dos objetos"]),
    ("Tamaño mínimo de objeto (% tamaño de pantalla)", ["Minimum object size (% of screen size)", "Minimale Objektgröße (% der Bildschirmgröße)", "Minimalny rozmiar obiektu (% rozmiaru ekranu)", "Minimální velikost objektu (% velikosti obrazovky)", "Taille minimale des objets (% de la taille de l'écran)", "Tamanho mínimo do objeto (% do tamanho do ecrã)"]),
    ("… para reflexiones", ["… for reflections", "… für Spiegelungen", "… dla odbić", "… pro odrazy", "… pour les reflets", "… para reflexos"]),
    ("Reflexiones en tiempo real", ["Real-time reflections", "Echtzeit-Spiegelungen", "Odbicia w czasie rzeczywistym", "Odrazy v reálném čase", "Reflets en temps réel", "Reflexos em tempo real"]),
    ("Sistemas de partículas: activo", ["Particle systems: active", "Partikelsysteme: aktiv", "Systemy cząsteczek: aktywne", "Částicové systémy: aktivní", "Systèmes de particules : actifs", "Sistemas de partículas: ativo"]),
    ("Máx. partículas por emisor", ["Max. particles per emitter", "Max. Partikel pro Emitter", "Maks. cząsteczek na emiter", "Max. částic na emitor", "Particules max. par émetteur", "Máx. partículas por emissor"]),
    ("Partículas: solo vehículo del usuario", ["Particles: user vehicle only", "Partikel: nur Fahrzeug des Nutzers", "Cząsteczki: tylko pojazd użytkownika", "Částice: pouze vozidlo uživatele", "Particules : véhicule de l'utilisateur uniquement", "Partículas: só veículo do utilizador"]),
    ("Partículas: sin P.S. en reflexiones", ["Particles: no P.S. in reflections", "Partikel: kein P.S. in Spiegelungen", "Cząsteczki: bez P.S. w odbiciach", "Částice: bez P.S. v odrazech", "Particules : pas de P.S. dans les reflets", "Partículas: sem P.S. nos reflexos"]),
    ("Efecto de brillo solar", ["Sun glow effect", "Sonnenglanz-Effekt", "Efekt blasku słońca", "Efekt slunečního jasu", "Effet d'éclat du soleil", "Efeito de brilho solar"]),
    ("Complejidad máxima de objetos (0-3)", ["Maximum object complexity (0-3)", "Maximale Objektkomplexität (0-3)", "Maksymalna złożoność obiektów (0-3)", "Maximální složitost objektů (0-3)", "Complexité maximale des objets (0-3)", "Complexidade máxima dos objetos (0-3)"]),
    ("Complejidad máxima del mapa (0-2)", ["Maximum map complexity (0-2)", "Maximale Kartenkomplexität (0-2)", "Maksymalna złożoność mapy (0-2)", "Maximální složitost mapy (0-2)", "Complexité maximale de la carte (0-2)", "Complexidade máxima do mapa (0-2)"]),
    ("Efectos de búfer stencil", ["Stencil buffer effects", "Stencil-Puffer-Effekte", "Efekty bufora stencil", "Efekty stencil bufferu", "Effets de tampon stencil", "Efeitos de buffer stencil"]),
    ("Sombras", ["Shadows", "Schatten", "Cienie", "Stíny", "Ombres", "Sombras"]),
    ("Reflexiones de lluvia", ["Rain reflections", "Regenspiegelungen", "Odbicia deszczu", "Odrazy deště", "Reflets de pluie", "Reflexos de chuva"]),
    ("Personas visibles en reflexiones de lluvia", ["People visible in rain reflections", "Personen in Regenspiegelungen sichtbar", "Ludzie widoczni w odbiciach deszczu", "Lidé viditelní v odrazech deště", "Personnes visibles dans les reflets de pluie", "Pessoas visíveis nos reflexos de chuva"]),
    ("Relación de pantalla manual", ["Manual screen ratio", "Manuelles Seitenverhältnis", "Ręczne proporcje ekranu", "Ruční poměr stran obrazovky", "Rapport d'écran manuel", "Proporção de ecrã manual"]),
    ("Modo económico de reflexiones: activar por debajo de FPS", ["Economy reflections mode: enable below FPS", "Sparmodus Spiegelungen: aktivieren unter FPS", "Tryb ekonomiczny odbić: włącz poniżej FPS", "Úsporný režim odrazů: zapnout pod FPS", "Mode reflets économique : activer sous FPS", "Modo económico de reflexos: ativar abaixo de FPS"]),
    ("Modo económico de reflexiones: desactivar por encima de FPS", ["Economy reflections mode: disable above FPS", "Sparmodus Spiegelungen: deaktivieren über FPS", "Tryb ekonomiczny odbić: wyłącz powyżej FPS", "Úsporný režim odrazů: vypnout nad FPS", "Mode reflets économique : désactiver au-dessus de FPS", "Modo económico de reflexos: desativar acima de FPS"]),
    ("Reducción dinámica de losas: reducir por debajo de FPS", ["Dynamic tile reduction: reduce below FPS", "Dynamische Kachelreduzierung: reduzieren unter FPS", "Dynamiczna redukcja kafelków: zmniejsz poniżej FPS", "Dynamické snížení dlaždic: snížit pod FPS", "Réduction dynamique des dalles : réduire sous FPS", "Redução dinâmica de blocos: reduzir abaixo de FPS"]),
    ("Reducción dinámica de losas: aumentar por encima de FPS", ["Dynamic tile reduction: increase above FPS", "Dynamische Kachelreduzierung: erhöhen über FPS", "Dynamiczna redukcja kafelków: zwiększ powyżej FPS", "Dynamické snížení dlaždic: zvýšit nad FPS", "Réduction dynamique des dalles : augmenter au-dessus de FPS", "Redução dinâmica de blocos: aumentar acima de FPS"]),
    ("Usar solo texturas de baja resolución", ["Use low-resolution textures only", "Nur Texturen mit niedriger Auflösung verwenden", "Używaj tylko tekstur niskiej rozdzielczości", "Používat pouze textury s nízkým rozlišením", "Utiliser uniquement des textures basse résolution", "Usar só texturas de baixa resolução"]),
    ("Limitación de todas las texturas (excepto las del propio autobús) a 256 px", [
        "Limit all textures (except the bus's own) to 256 px",
        "Alle Texturen (außer denen des eigenen Busses) auf 256 px begrenzen",
        "Ogranicz wszystkie tekstury (poza własnym autobusem) do 256 px",
        "Omezit všechny textury (kromě vlastního autobusu) na 256 px",
        "Limiter toutes les textures (sauf celles du bus) à 256 px",
        "Limitar todas as texturas (exceto as do próprio autocarro) a 256 px",
    ]),
    ("Mem. máx. de texturas para texturas de alta resolución (MB)", [
        "Max. texture memory for high-resolution textures (MB)",
        "Max. Texturspeicher für hochauflösende Texturen (MB)",
        "Maks. pamięć tekstur dla tekstur wysokiej rozdzielczości (MB)",
        "Max. paměť textur pro textury s vysokým rozlišením (MB)",
        "Mém. max. de textures pour les textures haute résolution (Mo)",
        "Mem. máx. de texturas para texturas de alta resolução (MB)",
    ]),
    ("Tamaño de textura de reflexiones en tiempo real", ["Real-time reflection texture size", "Texturgröße der Echtzeit-Spiegelungen", "Rozmiar tekstury odbić w czasie rzeczywistym", "Velikost textury odrazů v reálném čase", "Taille de texture des reflets en temps réel", "Tamanho da textura dos reflexos em tempo real"]),
    ("Canal de material: Night Map", ["Material channel: Night Map", "Materialkanal: Night Map", "Kanał materiału: Night Map", "Kanál materiálu: Night Map", "Canal de matériau : Night Map", "Canal de material: Night Map"]),
    ("Canal de material: Light Map", ["Material channel: Light Map", "Materialkanal: Light Map", "Kanał materiału: Light Map", "Kanál materiálu: Light Map", "Canal de matériau : Light Map", "Canal de material: Light Map"]),
    ("Canal de material: Light Map of Terrain", ["Material channel: Light Map of Terrain", "Materialkanal: Light Map of Terrain", "Kanał materiału: Light Map of Terrain", "Kanál materiálu: Light Map of Terrain", "Canal de matériau : Light Map of Terrain", "Canal de material: Light Map of Terrain"]),
    ("Canal de material: Reflexion Map", ["Material channel: Reflexion Map", "Materialkanal: Reflexion Map", "Kanał materiału: Reflexion Map", "Kanál materiálu: Reflexion Map", "Canal de matériau : Reflexion Map", "Canal de material: Reflexion Map"]),
    ("Canal de material: Bump Map", ["Material channel: Bump Map", "Materialkanal: Bump Map", "Kanał materiału: Bump Map", "Kanál materiálu: Bump Map", "Canal de matériau : Bump Map", "Canal de material: Bump Map"]),
    ("Volumen general", ["Master volume", "Gesamtlautstärke", "Głośność ogólna", "Celková hlasitost", "Volume général", "Volume geral"]),
    ("Efecto estéreo", ["Stereo effect", "Stereo-Effekt", "Efekt stereo", "Stereo efekt", "Effet stéréo", "Efeito estéreo"]),
    ("Recuento máximo de sonidos", ["Maximum sound count", "Maximale Anzahl Klänge", "Maksymalna liczba dźwięków", "Maximální počet zvuků", "Nombre maximal de sons", "Número máximo de sons"]),
    ("Sonidos de vehículos IA activos", ["AI vehicle sounds active", "KI-Fahrzeuggeräusche aktiv", "Dźwięki pojazdów AI aktywne", "Zvuky vozidel AI aktivní", "Sons des véhicules IA actifs", "Sons de veículos IA ativos"]),
    ("Sonidos del entorno activos", ["Scenery sounds active", "Umgebungsgeräusche aktiv", "Dźwięki otoczenia aktywne", "Zvuky okolí aktivní", "Sons de l'environnement actifs", "Sons do ambiente ativos"]),
    ("Efecto Doppler", ["Doppler effect", "Doppler-Effekt", "Efekt Dopplera", "Dopplerův efekt", "Effet Doppler", "Efeito Doppler"]),
    ("Efectos de reverberación", ["Reverb effects", "Hall-Effekte", "Efekty pogłosu", "Efekty dozvuku", "Effets de réverbération", "Efeitos de reverberação"]),
    ("Recuento máx. de tráfico rodado", ["Max. road traffic count", "Max. Anzahl Straßenverkehr", "Maks. liczba ruchu drogowego", "Max. počet silničního provozu", "Nombre max. de trafic routier", "Contagem máx. de tráfego rodoviário"]),
    ("Programado (ej.: autobuses)", ["Scheduled (e.g. buses)", "Fahrplangebunden (z. B. Busse)", "Według rozkładu (np. autobusy)", "Dle jízdního řádu (např. autobusy)", "Programmé (ex. : bus)", "Programado (ex.: autocarros)"]),
    ("Factor de tráfico en la carretera (%)", ["Road traffic factor (%)", "Verkehrsfaktor auf der Straße (%)", "Współczynnik ruchu na drodze (%)", "Faktor provozu na silnici (%)", "Facteur de trafic sur la route (%)", "Fator de tráfego na estrada (%)"]),
    ("Prioridad del horario", ["Timetable priority", "Fahrplan-Priorität", "Priorytet rozkładu", "Priorita jízdního řádu", "Priorité de l'horaire", "Prioridade do horário"]),
    ("Coches estacionados (%)", ["Parked cars (%)", "Geparkte Autos (%)", "Zaparkowane samochody (%)", "Zaparkovaná auta (%)", "Voitures stationnées (%)", "Carros estacionados (%)"]),
    ("Usar lista de IA reducida (menos tipos diferentes)", ["Use reduced AI list (fewer different types)", "Reduzierte KI-Liste verwenden (weniger Typen)", "Użyj zredukowanej listy AI (mniej typów)", "Použít zkrácený seznam AI (méně typů)", "Utiliser la liste IA réduite (moins de types)", "Usar lista de IA reduzida (menos tipos)"]),
    ("Recuento máx. de personas", ["Max. people count", "Max. Anzahl Personen", "Maks. liczba osób", "Max. počet osob", "Nombre max. de personnes", "Contagem máx. de pessoas"]),
    ("Factor de pasajeros en estaciones (%)", ["Passenger factor at stops (%)", "Fahrgastfaktor an Haltestellen (%)", "Współczynnik pasażerów na przystankach (%)", "Faktor cestujících na zastávkách (%)", "Facteur de passagers aux arrêts (%)", "Fator de passageiros nas paragens (%)"]),
    ("Azul", ["Blue", "Blau", "Niebieski", "Modrý", "Bleu", "Azul"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_languages_have_a_text_for_every_row() {
        for code in ["HUN", "SRB"] {
            let source = if code == "HUN" { include_str!("lang/HUN.txt") } else { include_str!("lang/SRB.txt") };
            let lines: Vec<&str> = source.lines().collect();
            assert_eq!(lines.len(), ROWS.len(), "{code} has a different number of lines than ROWS");
            for ((k, _), t) in ROWS.iter().zip(lines) {
                assert!(!t.trim().is_empty(), "{code}: empty text for {k}");
                assert_eq!(unescape(t).matches("{}").count(), k.matches("{}").count(), "{code}: placeholder mismatch in {k}");
            }
            assert!(embedded(code).is_some_and(|t| t.len() == ROWS.len()));
        }
    }

    #[test]
    fn rows_are_unique_and_complete() {
        let mut seen = std::collections::HashSet::new();
        for (k, tr) in ROWS {
            assert!(seen.insert(*k), "duplicate key {k}");
            let n = k.matches("{}").count();
            for t in tr {
                assert!(!t.is_empty(), "empty translation for {k}");
                assert_eq!(t.matches("{}").count(), n, "placeholder mismatch in {k}");
            }
        }
    }

    #[test]
    fn trf_fills_placeholders() {
        set_lang(Lang::De);
        assert_eq!(trf("Carpeta: {}", &["C:\\x"]), "Ordner: C:\\x");
        set_lang(Lang::Es);
        assert_eq!(trf("Carpeta: {}", &["a"]), "Carpeta: a");
    }
}

#[cfg(test)]
mod external_tests {
    use super::*;

    #[test]
    fn a_language_file_adds_a_language() {
        let dir = std::env::temp_dir().join(format!("omsi2l-lang-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
        unsafe { std::env::set_var("APPDATA", &dir) };
        let lang_dir = lang_dir();
        std::fs::create_dir_all(&lang_dir).unwrap();
        std::fs::write(lang_dir.join("CHN.txt"), "@name=中文\n\n> Listo.\n= 就绪。\n\n> Carpeta: {}\n= 文件夹：{}\n").unwrap();
        // the template lists every text with its English version and can be read back as a language
        let template = write_template().unwrap();
        assert!(std::fs::read_to_string(&template).unwrap().contains("> Listo.\n= Ready."));
        assert!(available(None).contains(&Lang::from_code("chn")));
        let chn = Lang::from_code("CHN");
        assert_eq!(chn.name(), "中文");
        set_lang(chn);
        assert_eq!(tr("Listo."), "就绪。");
        assert_eq!(trf("Carpeta: {}", &["C:\\x"]), "文件夹：C:\\x");
        // a text the file does not have shows in English, not Spanish
        assert_eq!(tr("Cerrar"), "Close");
        // the language of an older settings file
        assert_eq!(Lang::from_code("es"), Lang::Es);
        assert_eq!(Lang::from_code("PT"), Lang::Pt);
        set_lang(Lang::Es);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
