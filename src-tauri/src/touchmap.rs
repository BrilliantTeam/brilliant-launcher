use crate::instance::Instance;
use crate::paths;
use std::collections::HashMap;

const DATA_COMPONENTS: &str = "net.minecraft.core.component.DataComponents";
const USE_ANIMATION: &str = "net.minecraft.world.item.ItemUseAnimation";
const COMPONENT_GETTER: &str = "net.minecraft.core.component.DataComponentGetter";

struct Member {
    owner: &'static str,
    name: &'static str,
    args: Option<&'static str>,
    ret: &'static str,
}

const fn field(owner: &'static str, name: &'static str, ret: &'static str) -> Member {
    Member { owner, name, args: None, ret }
}

const fn method(owner: &'static str, name: &'static str, args: &'static str, ret: &'static str) -> Member {
    Member { owner, name, args: Some(args), ret }
}

enum Entry {
    Class(&'static str),
    Name(Member),
    Desc(Member),
    Text(&'static str),
}

fn layout(anim: &'static str, holder: Option<&'static str>) -> Vec<Entry> {
    use Entry::*;
    let mc = "net.minecraft.client.Minecraft";
    let hit = "net.minecraft.world.phys.HitResult";
    let hit_type = "net.minecraft.world.phys.HitResult$Type";
    let stack = "net.minecraft.world.item.ItemStack";
    let food = "net.minecraft.world.food.FoodProperties";
    let living = "net.minecraft.world.entity.LivingEntity";
    let mut entries = vec![
        Class(mc),
        Class(hit),
        Class(hit_type),
        Class("net.minecraft.client.player.LocalPlayer"),
        Class(stack),
        Class(anim),
        Class(food),
        Name(method(mc, "getInstance", "", mc)),
        Desc(method(mc, "getInstance", "", mc)),
        Name(field(mc, "hitResult", hit)),
        Desc(field(mc, "hitResult", hit)),
        Name(field(mc, "player", "net.minecraft.client.player.LocalPlayer")),
        Desc(field(mc, "player", "net.minecraft.client.player.LocalPlayer")),
        Name(method(hit, "getType", "", hit_type)),
        Desc(method(hit, "getType", "", hit_type)),
        Name(method(living, "getMainHandItem", "", stack)),
        Desc(method(living, "getMainHandItem", "", stack)),
        Name(method(living, "getOffhandItem", "", stack)),
        Name(method("net.minecraft.world.entity.player.Player", "canEat", "boolean", "boolean")),
        Name(method(stack, "getUseAnimation", "", anim)),
        Desc(method(stack, "getUseAnimation", "", anim)),
        Name(field(anim, "NONE", anim)),
        Desc(field(anim, "NONE", anim)),
        Name(field(hit_type, "ENTITY", hit_type)),
        Desc(field(hit_type, "ENTITY", hit_type)),
        Name(method(food, "canAlwaysEat", "", "boolean")),
    ];
    if let Some(holder) = holder {
        let kind = "net.minecraft.core.component.DataComponentType";
        entries.extend([
            Text("A"),
            Class(DATA_COMPONENTS),
            Name(field(DATA_COMPONENTS, "FOOD", kind)),
            Desc(field(DATA_COMPONENTS, "FOOD", kind)),
            Name(method(holder, "get", kind, "java.lang.Object")),
            Desc(method(holder, "get", kind, "java.lang.Object")),
        ]);
    } else {
        let item = "net.minecraft.world.item.Item";
        entries.extend([
            Text("B"),
            Class(item),
            Name(method(stack, "getItem", "", item)),
            Desc(method(stack, "getItem", "", item)),
            Name(method(item, "getFoodProperties", "", food)),
            Desc(method(item, "getFoodProperties", "", food)),
        ]);
    }
    entries
}

#[derive(Default)]
struct Official {
    classes: HashMap<String, String>,
    members: HashMap<(String, String, String), String>,
}

fn parse_official(text: &str) -> Official {
    let mut out = Official::default();
    let mut owner = String::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let Some((left, obf)) = line.trim().split_once(" -> ") else { continue };
        if !line.starts_with(' ') {
            owner = left.to_string();
            out.classes.insert(owner.clone(), obf.trim_end_matches(':').to_string());
            continue;
        }
        let left = left.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':');
        let Some((_, signature)) = left.split_once(' ') else { continue };
        let (name, args) = match signature.split_once('(') {
            Some((name, rest)) => (name, rest.rsplit_once(')').map_or(rest, |(args, _)| args).to_string()),
            None => (signature, "-".to_string()),
        };
        out.members.insert((owner.clone(), name.to_string(), args), obf.to_string());
    }
    out
}

fn descriptor(ty: &str, class: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    Some(match ty {
        "" | "void" => "V".into(),
        "boolean" => "Z".into(),
        "int" => "I".into(),
        _ => format!("L{};", class(ty)?.replace('.', "/")),
    })
}

fn member_desc(member: &Member, class: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let ret = descriptor(member.ret, class)?;
    Some(match member.args {
        None => ret,
        Some(args) => {
            let args = args.split(',').filter(|a| !a.is_empty()).map(|a| descriptor(a, class)).collect::<Option<String>>()?;
            format!("({args}){ret}")
        }
    })
}

#[derive(Default)]
struct Renames {
    classes: HashMap<String, String>,
    fields: HashMap<(String, String), String>,
    methods: HashMap<(String, String, String), String>,
}

fn parse_tiny(text: &str) -> Renames {
    let mut out = Renames::default();
    let mut owner = String::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        match parts.as_slice() {
            ["CLASS", obf, named, ..] | ["c", obf, named, ..] => {
                owner = obf.to_string();
                out.classes.insert(obf.to_string(), named.to_string());
            }
            ["FIELD", owner, _, obf, named, ..] => {
                out.fields.insert((owner.to_string(), obf.to_string()), named.to_string());
            }
            ["METHOD", owner, desc, obf, named, ..] => {
                out.methods.insert((owner.to_string(), obf.to_string(), desc.to_string()), named.to_string());
            }
            ["", "f", _, obf, named, ..] => {
                out.fields.insert((owner.clone(), obf.to_string()), named.to_string());
            }
            ["", "m", desc, obf, named, ..] => {
                out.methods.insert((owner.clone(), obf.to_string(), desc.to_string()), named.to_string());
            }
            _ => {}
        }
    }
    out
}

fn parse_tsrg(text: &str) -> Renames {
    let mut out = Renames::default();
    let mut owner = String::new();
    for line in text.lines().skip_while(|l| l.starts_with("tsrg2")) {
        if line.starts_with("\t\t") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if !line.starts_with('\t') {
            if let [obf, named, ..] = parts.as_slice() {
                owner = obf.to_string();
                out.classes.insert(obf.to_string(), named.to_string());
            }
            continue;
        }
        match parts.as_slice() {
            [obf, desc, named, ..] if desc.starts_with('(') => {
                out.methods.insert((owner.clone(), obf.to_string(), desc.to_string()), named.to_string());
            }
            [obf, named, ..] => {
                out.fields.insert((owner.clone(), obf.to_string()), named.to_string());
            }
            _ => {}
        }
    }
    out
}

enum Space<'a> {
    Official,
    Obfuscated,
    Renamed(&'a Renames, bool),
}

fn table(official: &Official, space: &Space) -> Option<String> {
    let anim = if official.classes.contains_key(USE_ANIMATION) { USE_ANIMATION } else { "net.minecraft.world.item.UseAnim" };
    let obf_class = |name: &str| -> Option<String> {
        if name == "java.lang.Object" {
            return Some(name.into());
        }
        official.classes.get(name).cloned()
    };
    let class = |name: &str| -> Option<String> {
        match space {
            Space::Official => obf_class(name).map(|_| name.to_string()),
            Space::Obfuscated => obf_class(name),
            Space::Renamed(renames, keep_official) => {
                let obf = obf_class(name)?;
                if obf == "java.lang.Object" {
                    return Some(obf);
                }
                let named = renames.classes.get(&obf.replace('.', "/"))?;
                Some(if *keep_official && named.starts_with("net/minecraft/src/C_") { name.to_string() } else { named.replace('/', ".") })
            }
        }
    };
    let mut out = Vec::new();
    let holder = match (official.classes.contains_key(DATA_COMPONENTS), official.classes.contains_key(COMPONENT_GETTER)) {
        (false, _) => None,
        (true, true) => Some(COMPONENT_GETTER),
        (true, false) => Some("net.minecraft.core.component.DataComponentHolder"),
    };
    for entry in layout(anim, holder) {
        out.push(match entry {
            Entry::Class(name) => class(name)?,
            Entry::Text(text) => text.to_string(),
            Entry::Desc(member) => member_desc(&member, &class)?,
            Entry::Name(member) => {
                let key = (member.owner.to_string(), member.name.to_string(), member.args.unwrap_or("-").to_string());
                let obf = official.members.get(&key)?;
                match space {
                    Space::Official => member.name.to_string(),
                    Space::Obfuscated => obf.clone(),
                    Space::Renamed(renames, _) => {
                        let owner = obf_class(member.owner)?.replace('.', "/");
                        match member.args {
                            None => renames.fields.get(&(owner, obf.clone()))?.clone(),
                            Some(_) => renames.methods.get(&(owner, obf.clone(), member_desc(&member, &obf_class)?))?.clone(),
                        }
                    }
                }
            }
        });
    }
    Some(out.join("~"))
}

fn zip_entry(path: &std::path::Path, name: &str) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(name).ok()?, &mut text).ok()?;
    Some(text)
}

fn mcp_config(mc: &str) -> Option<std::path::PathBuf> {
    let dir = paths::libraries().join("de/oceanlabs/mcp/mcp_config");
    let version = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|v| v == mc || v.starts_with(&format!("{mc}-"))).max()?;
    Some(dir.join(&version).join(format!("mcp_config-{version}.zip")))
}

fn tables(official: &Official, loader: &str, mc: &str) -> Vec<String> {
    let renamed = |renames: Option<Renames>, keep: bool| renames.and_then(|r| table(official, &Space::Renamed(&r, keep)));
    let srg = || renamed(mcp_config(mc).and_then(|p| zip_entry(&p, "config/joined.tsrg")).map(|t| parse_tsrg(&t)), true);
    let candidates = match loader {
        "fabric" | "quilt" => {
            let jar = paths::libraries().join(format!("net/fabricmc/intermediary/{mc}/intermediary-{mc}.jar"));
            vec![renamed(zip_entry(&jar, "mappings/mappings.tiny").map(|t| parse_tiny(&t)), false)]
        }
        "forge" => vec![srg(), table(official, &Space::Official)],
        "neoforge" => vec![table(official, &Space::Official), srg()],
        _ => vec![table(official, &Space::Obfuscated)],
    };
    candidates.into_iter().flatten().collect()
}

pub async fn jvm_arg(instance: &Instance, mirror: bool) -> Option<String> {
    let cache = paths::root().join("cache").join("touch").join(format!("{}-{}.txt", instance.mc_version, instance.loader));
    let value = match tokio::fs::read_to_string(&cache).await {
        Ok(value) => value,
        Err(_) => {
            let json: serde_json::Value = serde_json::from_slice(&tokio::fs::read(paths::version_json(&instance.mc_version)).await.ok()?).ok()?;
            let mappings = &json["downloads"]["client_mappings"];
            let value = match mappings["url"].as_str() {
                None => String::new(),
                Some(url) => {
                    let bytes = crate::net::get_bytes(url, mirror).await.ok()?;
                    if mappings["sha1"].as_str().is_some_and(|want| !crate::net::sha1_hex(&bytes).eq_ignore_ascii_case(want)) {
                        return None;
                    }
                    let (loader, mc) = (instance.loader.clone(), instance.mc_version.clone());
                    tokio::task::spawn_blocking(move || tables(&parse_official(&String::from_utf8_lossy(&bytes)), &loader, &mc).join("|")).await.ok()?
                }
            };
            let _ = tokio::fs::create_dir_all(cache.parent()?).await;
            let _ = tokio::fs::write(&cache, &value).await;
            value
        }
    };
    (!value.is_empty()).then(|| format!("-Dbrilliant.touch={value}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFICIAL: &str = "# header
net.minecraft.client.Minecraft -> enn:
    net.minecraft.client.player.LocalPlayer player -> t
    net.minecraft.world.phys.HitResult hitResult -> w
    2465:2465:net.minecraft.client.Minecraft getInstance() -> N
net.minecraft.client.player.LocalPlayer -> fiy:
net.minecraft.world.phys.HitResult -> eeg:
    1:1:net.minecraft.world.phys.HitResult$Type getType() -> c
net.minecraft.world.phys.HitResult$Type -> eeg$a:
    net.minecraft.world.phys.HitResult$Type ENTITY -> c
net.minecraft.world.entity.LivingEntity -> bfz:
    1:1:net.minecraft.world.item.ItemStack getMainHandItem() -> eR
    1:1:net.minecraft.world.item.ItemStack getOffhandItem() -> eS
net.minecraft.world.entity.player.Player -> byo:
    1:1:boolean canEat(boolean) -> q
net.minecraft.world.item.ItemStack -> cfz:
    1:1:net.minecraft.world.item.UseAnim getUseAnimation() -> v
    1:1:net.minecraft.world.item.Item getItem() -> d
net.minecraft.world.item.UseAnim -> cgx:
    net.minecraft.world.item.UseAnim NONE -> a
net.minecraft.world.food.FoodProperties -> bxm:
    1:1:boolean canAlwaysEat() -> d
net.minecraft.world.item.Item -> cfu:
    1:1:net.minecraft.world.food.FoodProperties getFoodProperties() -> w
";

    #[test]
    fn official_mappings_become_runtime_names() {
        let official = parse_official(OFFICIAL);
        let obf = table(&official, &Space::Obfuscated).unwrap();
        assert!(obf.starts_with("enn~eeg~eeg$a~fiy~cfz~cgx~bxm~N~()Lenn;~w~Leeg;~t~Lfiy;~c~()Leeg$a;~eR~()Lcfz;~eS~q~v~()Lcgx;~a~Lcgx;~c~Leeg$a;~d~B~cfu~d~()Lcfu;~w~()Lbxm;"), "{obf}");
        let mojang = table(&official, &Space::Official).unwrap();
        assert!(mojang.contains("net.minecraft.world.item.UseAnim~") && mojang.contains("getMainHandItem~()Lnet/minecraft/world/item/ItemStack;"), "{mojang}");
        let tiny = parse_tiny("v1\tofficial\tintermediary\nCLASS\tenn\tnet/minecraft/class_310\nCLASS\teeg\tnet/minecraft/class_239\nCLASS\teeg$a\tnet/minecraft/class_239$class_240\nCLASS\tfiy\tnet/minecraft/class_746\nCLASS\tcfz\tnet/minecraft/class_1799\nCLASS\tcgx\tnet/minecraft/class_1839\nCLASS\tbxm\tnet/minecraft/class_4174\nCLASS\tbfz\tnet/minecraft/class_1309\nCLASS\tbyo\tnet/minecraft/class_1657\nCLASS\tcfu\tnet/minecraft/class_1792\nMETHOD\tenn\t()Lenn;\tN\tmethod_1551\nFIELD\tenn\tLeeg;\tw\tfield_1765\nFIELD\tenn\tLfiy;\tt\tfield_1724\nMETHOD\teeg\t()Leeg$a;\tc\tmethod_17783\nMETHOD\tbfz\t()Lcfz;\teR\tmethod_6047\nMETHOD\tbfz\t()Lcfz;\teS\tmethod_6079\nMETHOD\tbyo\t(Z)Z\tq\tmethod_7332\nMETHOD\tcfz\t()Lcgx;\tv\tmethod_7976\nFIELD\tcgx\tLcgx;\ta\tfield_8952\nFIELD\teeg$a\tLeeg$a;\tc\tfield_1331\nMETHOD\tbxm\t()Z\td\tmethod_19233\nMETHOD\tcfz\t()Lcfu;\td\tmethod_7909\nMETHOD\tcfu\t()Lbxm;\tw\tmethod_19264\n");
        let fabric = table(&official, &Space::Renamed(&tiny, false)).unwrap();
        assert!(fabric.starts_with("net.minecraft.class_310~net.minecraft.class_239~net.minecraft.class_239$class_240~") && fabric.contains("method_1551~()Lnet/minecraft/class_310;~field_1765"), "{fabric}");
        let srg = parse_tsrg("tsrg2 obf srg id\nenn net/minecraft/src/C_3391_ 3391\n\tw f_91077_ 91077\n\tt f_91074_ 91074\n\tN ()Lenn; m_91087_ 91087\n\t\tstatic\neeg net/minecraft/src/C_3531_ 3531\n\tc ()Leeg$a; m_6662_ 6662\neeg$a net/minecraft/src/C_3532_ 3532\n\tc f_82445_ 82445\nfiy net/minecraft/src/C_3899_ 3899\ncfz net/minecraft/src/C_1799_ 1799\n\tv ()Lcgx; m_41780_ 41780\n\td ()Lcfu; m_41720_ 41720\ncgx net/minecraft/src/C_1859_ 1859\n\ta f_42016_ 42016\nbxm net/minecraft/src/C_2192_ 2192\n\td ()Z m_38749_ 38749\nbfz net/minecraft/src/C_1309_ 1309\n\teR ()Lcfz; m_21205_ 21205\n\teS ()Lcfz; m_21206_ 21206\nbyo net/minecraft/src/C_1657_ 1657\n\tq (Z)Z m_36391_ 36391\ncfu net/minecraft/src/C_1792_ 1792\n\tw ()Lbxm; m_41473_ 41473\n");
        let forge = table(&official, &Space::Renamed(&srg, true)).unwrap();
        assert!(forge.starts_with("net.minecraft.client.Minecraft~") && forge.contains("m_91087_~()Lnet/minecraft/client/Minecraft;~f_91077_"), "{forge}");
        assert!([&obf, &mojang, &fabric, &forge].iter().all(|t| !t.contains([',', '-', '=', ' '])), "Pojav 的 JVM 參數解析遇到逗號會把參數併進前一個");
        assert!(table(&parse_official("net.minecraft.client.Minecraft -> enn:\n"), &Space::Obfuscated).is_none());
    }

    #[test]
    #[ignore]
    fn real_mappings() {
        let dir = std::path::PathBuf::from(std::env::var("TOUCHMAP_DIR").unwrap());
        let official = parse_official(&std::fs::read_to_string(dir.join("client.txt")).unwrap());
        println!("obf {:?}", table(&official, &Space::Obfuscated));
        println!("mojang {:?}", table(&official, &Space::Official));
        if let Ok(tiny) = std::fs::read_to_string(dir.join("mappings.tiny")) {
            println!("fabric {:?}", table(&official, &Space::Renamed(&parse_tiny(&tiny), false)));
        }
        if let Ok(tsrg) = std::fs::read_to_string(dir.join("joined.tsrg")) {
            println!("forge {:?}", table(&official, &Space::Renamed(&parse_tsrg(&tsrg), true)));
        }
    }
}
