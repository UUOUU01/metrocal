//! 本地化：简体中文 / English / Français。
//! Localization: simplified Chinese / English / French.

use crate::engine::num_to_string;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    ZhCn,
    En,
    Fr,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::ZhCn, Lang::En, Lang::Fr];

    pub fn code(self) -> &'static str {
        match self {
            Lang::ZhCn => "zh-CN",
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }

    pub fn from_code(s: &str) -> Lang {
        match s {
            "zh-CN" | "zh" => Lang::ZhCn,
            "fr" => Lang::Fr,
            _ => Lang::En,
        }
    }

    pub fn native(self) -> &'static str {
        match self {
            Lang::ZhCn => "中文",
            Lang::En => "English",
            Lang::Fr => "Français",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Lang::ZhCn => "中",
            Lang::En => "EN",
            Lang::Fr => "FR",
        }
    }
}

pub fn dec_sep(lang: Lang) -> &'static str {
    match lang {
        Lang::Fr => ",",
        _ => ".",
    }
}

pub fn group_sep(lang: Lang) -> &'static str {
    match lang {
        Lang::Fr => "\u{a0}", // no-break space, widely supported glyph
        _ => ",",
    }
}

pub struct Strings {
    pub app_name: &'static str,
    pub tagline: &'static str,
    pub nav_calc: &'static str,
    pub nav_help: &'static str,
    pub nav_settings: &'static str,
    pub history: &'static str,
    pub history_empty: &'static str,
    pub clear: &'static str,
    pub theme: &'static str,
    pub dark: &'static str,
    pub light: &'static str,
    pub language: &'static str,
    pub accent: &'static str,
    pub accents: [&'static str; 8],
    pub data: &'static str,
    pub history_count: &'static str,
    pub settings_path: &'static str,
    pub err_div0: &'static str,
    pub err_invalid: &'static str,
    pub err_overflow: &'static str,
    pub about_line: &'static str,
    pub safety_line: &'static str,
    pub version_title: &'static str,
    // Windows 8 开始屏幕
    pub g_apps: &'static str,
    pub g_personal: &'static str,
    pub g_data: &'static str,
    pub t_calc: &'static str,
    pub t_history: &'static str,
    pub t_help: &'static str,
    pub t_theme: &'static str,
    pub t_lang: &'static str,
    pub t_accent: &'static str,
    pub t_clear: &'static str,
    pub t_settings: &'static str,
    pub t_exit: &'static str,
}

const ZH: Strings = Strings {
    app_name: "Metro 计算器",
    tagline: "光滑 · 快 · 现代",
    nav_calc: "计算器",
    nav_help: "帮助",
    nav_settings: "设置",
    history: "历史记录",
    history_empty: "暂无记录\n按下 = 之后，计算结果会出现在这里。\n点击任意记录可重新载入该结果。",
    clear: "清除",
    theme: "主题",
    dark: "深色",
    light: "浅色",
    language: "语言",
    accent: "强调色",
    accents: ["蓝", "青", "绿", "紫", "品红", "橙", "红", "靛"],
    data: "数据",
    history_count: "{} 条记录",
    settings_path: "设置与历史保存在本地：{}",
    err_div0: "不能除以零",
    err_invalid: "无效输入",
    err_overflow: "溢出",
    about_line: "Metro 计算器 1.0.0 · Rust + Skia · Windows / macOS / Linux",
    safety_line: "安全 Rust：unsafe 仅限 GPU 初始化的 2 行封装 · 不联网 · 数据仅存本机",
    version_title: "版本",
    g_apps: "应用",
    g_personal: "个性化",
    g_data: "数据",
    t_calc: "计算器",
    t_history: "历史记录",
    t_help: "帮助",
    t_theme: "深浅主题",
    t_lang: "切换语言",
    t_accent: "强调色",
    t_clear: "清除历史",
    t_settings: "全部设置",
    t_exit: "退出",
};

const EN: Strings = Strings {
    app_name: "Metro Calculator",
    tagline: "Smooth · Fast · Modern",
    nav_calc: "Calculator",
    nav_help: "Help",
    nav_settings: "Settings",
    history: "History",
    history_empty: "No history yet\nResults appear here after you press =.\nClick any entry to load it again.",
    clear: "Clear",
    theme: "Theme",
    dark: "Dark",
    light: "Light",
    language: "Language",
    accent: "Accent color",
    accents: ["Blue", "Teal", "Green", "Purple", "Magenta", "Orange", "Red", "Indigo"],
    data: "Data",
    history_count: "{} items",
    settings_path: "Settings & history stored locally at: {}",
    err_div0: "Cannot divide by zero",
    err_invalid: "Invalid input",
    err_overflow: "Overflow",
    about_line: "Metro Calculator 1.0.0 · Rust + Skia · Windows / macOS / Linux",
    safety_line: "safe Rust: unsafe only in 2 audited GL-init wrappers · no network · data stays local",
    version_title: "Version",
    g_apps: "Apps",
    g_personal: "Personalize",
    g_data: "Data",
    t_calc: "Calculator",
    t_history: "History",
    t_help: "Help",
    t_theme: "Theme",
    t_lang: "Language",
    t_accent: "Accent color",
    t_clear: "Clear history",
    t_settings: "All settings",
    t_exit: "Exit",
};

const FR: Strings = Strings {
    app_name: "Calculatrice Metro",
    tagline: "Fluide · Rapide · Moderne",
    nav_calc: "Calculatrice",
    nav_help: "Aide",
    nav_settings: "Réglages",
    history: "Historique",
    history_empty: "Aucun historique\nLes résultats apparaissent ici après =.\nCliquez sur une entrée pour la recharger.",
    clear: "Effacer",
    theme: "Thème",
    dark: "Sombre",
    light: "Clair",
    language: "Langue",
    accent: "Couleur d'accent",
    accents: ["Bleu", "Turquoise", "Vert", "Violet", "Magenta", "Orange", "Rouge", "Indigo"],
    data: "Données",
    history_count: "{} éléments",
    settings_path: "Paramètres et historique enregistrés dans : {}",
    err_div0: "Division par zéro",
    err_invalid: "Entrée non valide",
    err_overflow: "Dépassement",
    about_line: "Calculatrice Metro 1.0.0 · Rust + Skia · Windows / macOS / Linux",
    safety_line: "Rust sûr : unsafe limité à 2 encapsulations d'init GL auditées · sans réseau · données locales",
    version_title: "Version",
    g_apps: "Applications",
    g_personal: "Personnalisation",
    g_data: "Données",
    t_calc: "Calculatrice",
    t_history: "Historique",
    t_help: "Aide",
    t_theme: "Thème",
    t_lang: "Langue",
    t_accent: "Couleur d'accent",
    t_clear: "Effacer l'historique",
    t_settings: "Réglages",
    t_exit: "Quitter",
};

pub fn strings(lang: Lang) -> &'static Strings {
    match lang {
        Lang::ZhCn => &ZH,
        Lang::En => &EN,
        Lang::Fr => &FR,
    }
}

// ---------- help ----------

pub struct HelpSection {
    pub title: &'static str,
    pub body: &'static str,
}

const ZH_HELP: [HelpSection; 7] = [
    HelpSection {
        title: "欢迎使用",
        body: "一款遵循 Metro 设计语言的桌面计算器：大号排版、纯色磁贴、信息优先。\n启动时的动画会遮盖资源加载：磁贴依次滑入、按钮带按压回弹、页面切换平滑滑动。\n\n本应用完全使用安全 Rust 编写（crate 级禁止 unsafe），不访问网络，不收集任何数据。",
    },
    HelpSection {
        title: "基本使用",
        body: "• 数字 0–9 与小数点：输入当前数字（最多 16 位）\n• ÷ × − +：四则运算；连续按运算符可替换运算\n• =：得出结果。再次按 = 不做任何运算（幂等，不重复上次运算）\n• %：正常数学运算——把当前数字除以 100（200 → 2，5 → 0.05）；连按持续折算，绝不产生异常文本\n• ±：切换正负号\n• 1/x、x²、√：倒数、平方、平方根\n• CE：清除当前输入；C：全部清除；⌫：逐位退格\n• 出错时（如除以零）显示红色提示，任意数字键即可重新开始",
    },
    HelpSection {
        title: "键盘快捷键",
        body: "0–9：数字\n. 或 ,：小数点\n+ − * /：运算符（支持小键盘）\n%：百分比\nEnter 或 =：等于\nTab / Shift+Tab：在按键之间循环焦点\n← → ↑ ↓：按网格移动焦点\n空格：激活焦点所在按键\nBackspace：退格\nDelete：清除当前输入（CE）\nEsc：全部清除（C）；其他页面返回开始屏幕\nH：开/关历史记录面板\nHome：开始屏幕\nF1：打开帮助",
    },
    HelpSection {
        title: "历史记录面板",
        body: "点击顶栏的时钟图标开关右侧面板。面板保存最近 50 条计算记录，并随设置文件持久化。\n点击任意记录，会把该结果载入当前输入。点击面板顶部的“清除”可一键清空。",
    },
    HelpSection {
        title: "个性化设置",
        body: "主题：深色 / 浅色，即刻生效。\n强调色：8 种经典 Metro 纯色，数字磁贴与界面点缀随之变化。\n语言：中文 / English / Français，界面与帮助文档同步切换，数字分组与小数符号亦随本地习惯（法语使用逗号小数点）。\n设置与历史保存在本地配置目录，无需任何权限。",
    },
    HelpSection {
        title: "安全与隐私",
        body: "整个应用以 #![deny(unsafe_code)] 编译：除 gl_context.rs 中两个经过审计的 GL 初始化封装外，其余全部代码均为安全 Rust（GPU 不可用时的 CPU 兜底路径不涉及任何 unsafe）。\n计算引擎附带完整单元测试（cargo test 或 --selftest 可验证）。\n程序不访问网络、不读取剪贴板、不写除自身设置文件之外的任何位置。",
    },
    HelpSection {
        title: "关于",
        body: "Metro 计算器 1.0.0\n设计语言：Metro —— 源自地铁指示牌的启发：“光滑、快、现代”。\n技术栈：Rust + Skia（GPU 加速渲染，OpenGL 后端；无 GPU 时自动回退 CPU 光栅），跨 Windows / macOS / Linux。\n字体：自动探测系统中文字体（微软雅黑 / 苹方 / Noto Sans CJK），西文优先使用 Segoe UI Light。",
    },
];

const EN_HELP: [HelpSection; 7] = [
    HelpSection {
        title: "Welcome",
        body: "A desktop calculator in the Metro design language: large typography, flat color tiles, content first.\nAnimations mask loading: tiles slide in one by one, buttons spring back on press, pages glide horizontally.\n\nThe app is written in 100% safe Rust (unsafe is forbidden at crate level), never touches the network and collects nothing.",
    },
    HelpSection {
        title: "Basic usage",
        body: "• Digits 0–9 and the decimal point: type the current number (up to 16 digits)\n• ÷ × − + : arithmetic; pressing another operator replaces the pending one\n• = : evaluate. Pressing = again does nothing (idempotent — the last operation is never repeated)\n• % : plain math — divides the current number by 100 (200 → 2, 5 → 0.05); repeated presses keep dividing, never producing odd text\n• ± : toggle sign\n• 1/x, x², √ : reciprocal, square, square root\n• CE clears the entry, C clears everything, ⌫ erases one digit\n• On error (e.g. division by zero) a red message appears; any digit starts over",
    },
    HelpSection {
        title: "Keyboard shortcuts",
        body: "0–9 : digits\n. or , : decimal point\n+ − * / : operators (numpad supported)\n% : percentage\nEnter or = : equals\nTab / Shift+Tab : cycle focus across keys\n← → ↑ ↓ : move focus on the grid\nSpace : activate the focused key\nBackspace : erase last digit\nDelete : clear entry (CE)\nEsc : clear all (C); on other pages returns to Start\nH : toggle the history panel\nHome : Start screen\nF1 : open Help",
    },
    HelpSection {
        title: "History panel",
        body: "Click the clock icon in the top bar to toggle the side panel. It keeps the last 50 calculations and is persisted with your settings.\nClick an entry to load its result as the current input. Use Clear at the top of the panel to wipe it.",
    },
    HelpSection {
        title: "Personalization",
        body: "Theme: dark / light, applied instantly.\nAccent color: 8 classic Metro colors; digit tiles and highlights follow your choice.\nLanguage: 中文 / English / Français — UI and Help switch together, and number formatting follows the locale (French uses a comma decimal separator).\nSettings and history live in your local config folder; no permissions needed.",
    },
    HelpSection {
        title: "Safety & privacy",
        body: "The whole app is compiled with #![deny(unsafe_code)]: apart from two audited GL-initialization wrappers in gl_context.rs, every line is safe Rust (the CPU fallback path contains no unsafe at all).\nThe calculation engine ships with full unit tests (cargo test, or --selftest).\nThe app never accesses the network, never reads your clipboard, and writes nothing outside its own settings file.",
    },
    HelpSection {
        title: "About",
        body: "Metro Calculator 1.0.0\nDesign language: Metro — inspired by transit signage: “smooth, fast, modern”.\nStack: Rust + Skia (GPU-accelerated rendering, OpenGL backend; automatic CPU-raster fallback when no GPU context is available) across Windows / macOS / Linux.\nFonts: system CJK fonts are detected automatically (Microsoft YaHei / PingFang / Noto Sans CJK); Latin text prefers Segoe UI Light.",
    },
];

const FR_HELP: [HelpSection; 7] = [
    HelpSection {
        title: "Bienvenue",
        body: "Une calculatrice de bureau dans le langage de design Metro : grande typographie, tuiles colorées à plat, contenu d'abord.\nLes animations masquent le chargement : les tuiles glissent une à une, les boutons reviennent avec rebond, les pages se déplacent en douceur.\n\nL'application est écrite en Rust 100% sûr (unsafe interdit au niveau du crate), n'accède jamais au réseau et ne collecte rien.",
    },
    HelpSection {
        title: "Utilisation de base",
        body: "• Chiffres 0–9 et virgule : saisir le nombre courant (16 chiffres max)\n• ÷ × − + : opérations ; appuyer sur un autre opérateur remplace l'opérateur en attente\n• = : calculer. Appuyer encore sur = ne fait rien (idempotent — la dernière opération n'est jamais répétée)\n• % : mathématique simple — divise le nombre courant par 100 (200 → 2, 5 → 0,05) ; les appuis répétés continuent de diviser\n• ± : changer le signe\n• 1/x, x², √ : inverse, carré, racine carrée\n• CE efface la saisie, C efface tout, ⌫ efface un chiffre\n• En cas d'erreur (ex. division par zéro) un message rouge s'affiche ; n'importe quel chiffre recommence",
    },
    HelpSection {
        title: "Raccourcis clavier",
        body: "0–9 : chiffres\n. ou , : séparateur décimal\n+ − * / : opérateurs (pavé numérique accepté)\n% : pourcentage\nEntrée ou = : égal\nTab / Maj+Tab : parcourir les touches\n← → ↑ ↓ : déplacer le focus sur la grille\nEspace : activer la touche ciblée\nRetour arrière : effacer le dernier chiffre\nSuppr : effacer la saisie (CE)\nÉchap : tout effacer (C) ; sur les autres pages, retour à l'écran d'accueil\nH : afficher/masquer l'historique\nAccueil : écran d'accueil\nF1 : ouvrir l'aide",
    },
    HelpSection {
        title: "Panneau d'historique",
        body: "Cliquez sur l'horloge dans la barre supérieure pour afficher le panneau. Il garde les 50 derniers calculs, enregistrés avec vos paramètres.\nCliquez sur une entrée pour recharger son résultat. Utilisez Effacer en haut du panneau pour tout vider.",
    },
    HelpSection {
        title: "Personnalisation",
        body: "Thème : sombre / clair, appliqué instantanément.\nCouleur d'accent : 8 couleurs Metro classiques ; les tuiles de chiffres suivent votre choix.\nLangue : 中文 / English / Français — l'interface et l'aide basculent ensemble, et les nombres suivent la langue (le français utilise la virgule décimale).\nParamètres et historique sont stockés dans votre dossier de configuration local.",
    },
    HelpSection {
        title: "Sécurité et confidentialité",
        body: "Toute l'application est compilée avec #![deny(unsafe_code)] : hors deux encapsulations GL auditées dans gl_context.rs, chaque ligne est du Rust sûr (le chemin de secours CPU ne contient aucun unsafe).\nLe moteur de calcul est couvert par des tests unitaires (cargo test ou --selftest).\nL'application n'accède pas au réseau, ne lit pas le presse-papiers et n'écrit rien en dehors de son fichier de paramètres.",
    },
    HelpSection {
        title: "À propos",
        body: "Calculatrice Metro 1.0.0\nLangage de design : Metro — inspiré de la signalétique des transports : « fluide, rapide, moderne ».\nTechnique : Rust + Skia (rendu GPU accéléré, backend OpenGL ; repli CPU automatique sans contexte GPU) sous Windows / macOS / Linux.\nPolices : les polices CJK du système sont détectées automatiquement (Microsoft YaHei / PingFang / Noto Sans CJK) ; le latin préfère Segoe UI Light.",
    },
];

pub fn help_sections(lang: Lang) -> &'static [HelpSection] {
    match lang {
        Lang::ZhCn => &ZH_HELP,
        Lang::En => &EN_HELP,
        Lang::Fr => &FR_HELP,
    }
}

// ---------- number formatting ----------

fn group_digits(digits: &str, sep: &str) -> String {
    let n = digits.len();
    if n <= 3 || sep.is_empty() {
        return digits.to_owned();
    }
    let mut out = String::with_capacity(n + n / 3 * sep.len());
    let lead = n % 3;
    if lead > 0 {
        out.push_str(&digits[..lead]);
        out.push_str(sep);
    }
    let tail_len = n - lead;
    for (i, ch) in digits[lead..].chars().enumerate() {
        out.push(ch);
        if (i + 1) % 3 == 0 && i + 1 != tail_len {
            out.push_str(sep);
        }
    }
    out
}

/// Localize a canonical ASCII number string (grouping + decimal separator).
pub fn localize_number(s: &str, lang: Lang) -> String {
    if s.contains('e') {
        return s.to_owned(); // scientific notation stays universal
    }
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    if rest.is_empty() {
        return sign.to_owned();
    }
    let (ip, fp) = match rest.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let g = group_digits(ip, group_sep(lang));
    match fp {
        Some(fp) => format!("{sign}{g}{}{fp}", dec_sep(lang)),
        None => format!("{sign}{g}"),
    }
}

/// Format a value for display, localized.
/// 极大/极小值用科学计数法显示（仅显示层；entry/历史永远是纯数字格式）。
pub fn fmt_value(v: f64, lang: Lang) -> String {
    let a = v.abs();
    if a >= 1e16 || (v != 0.0 && a < 1e-6) {
        let s = format!("{v:.9e}");
        if let Some((m, ex)) = s.split_once('e') {
            let m = m.trim_end_matches('0').trim_end_matches('.');
            let exp: i32 = ex.parse().unwrap_or(0);
            return format!("{}e{}{}", m, if exp < 0 { "-" } else { "+" }, exp.abs());
        }
        return s;
    }
    localize_number(&num_to_string(v), lang)
}

/// Format a typed entry string (may keep a trailing separator, e.g. "0.").
pub fn fmt_entry(entry: &str, lang: Lang) -> String {
    if entry.is_empty() {
        return "0".to_owned();
    }
    let (sign, rest) = match entry.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", entry),
    };
    if rest.is_empty() {
        return "-".to_owned();
    }
    let (ip, fp) = match rest.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (rest, None),
    };
    let g = group_digits(ip, group_sep(lang));
    match fp {
        Some(fp) => format!("{sign}{g}{}{fp}", dec_sep(lang)),
        None => format!("{sign}{g}"),
    }
}

// ---------- verification ----------

pub fn parity_check() -> Result<(), String> {
    let zh = help_sections(Lang::ZhCn).len();
    let en = help_sections(Lang::En).len();
    let fr = help_sections(Lang::Fr).len();
    if zh != en || en != fr {
        return Err(format!("help sections mismatch: {zh}/{en}/{fr}"));
    }
    for s in help_sections(Lang::ZhCn)
        .iter()
        .chain(help_sections(Lang::En))
        .chain(help_sections(Lang::Fr))
    {
        if s.title.trim().is_empty() || s.body.trim().is_empty() {
            return Err("empty help section".to_owned());
        }
    }
    Ok(())
}

pub fn format_check() -> Result<(), String> {
    let eq = |a: &str, b: &str, what: &str| -> Result<(), String> {
        if a == b {
            Ok(())
        } else {
            Err(format!("{what}: {a:?} != {b:?}"))
        }
    };
    eq(&num_to_string(0.1), "0.1", "num 0.1")?;
    eq(&num_to_string(123456789.0), "123456789", "num int")?;
    eq(&num_to_string(1e20), "100000000000000000000", "num plain big")?;
    eq(&num_to_string(-0.0), "0", "num -0")?;
    eq(&num_to_string(2.025000013e-7), "0.0000002025", "num plain tiny")?;
    eq(&fmt_value(1e20, Lang::En), "1e+20", "display sci big")?;
    eq(&fmt_value(1234567.0, Lang::En), "1,234,567", "group en")?;
    eq(&fmt_value(1234567.0, Lang::Fr), "1\u{a0}234\u{a0}567", "group fr")?;
    eq(&fmt_value(0.5, Lang::Fr), "0,5", "dec fr")?;
    eq(&fmt_value(0.1, Lang::En), "0.1", "dec en")?;
    eq(&fmt_entry("1234.5", Lang::En), "1,234.5", "entry en")?;
    eq(&fmt_entry("0.", Lang::Fr), "0,", "entry fr dot")?;
    Ok(())
}
