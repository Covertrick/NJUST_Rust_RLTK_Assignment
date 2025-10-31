// 引入 Specs ECS 框架的预设模块
use specs::prelude::*;
// 引入 Specs 的序列化/反序列化相关工具
use specs::saveload::{SimpleMarker, SimpleMarkerAllocator, SerializeComponents, DeserializeComponents, MarkedBuilder};
// 引入 Specs 的错误类型
use specs::error::NoError;
// 引入自定义组件模块
use super::components::*;
// 文件操作相关标准库
use std::fs::File;
use std::path::Path;
use std::fs;

// 定义一个宏，用于序列化多个组件类型
macro_rules! serialize_individually {
    ($ecs:expr, $ser:expr, $data:expr, $( $type:ty),*) => {
        $(
        // 对每个组件类型进行序列化
        SerializeComponents::<NoError, SimpleMarker<SerializeMe>>::serialize(
            &( $ecs.read_storage::<$type>(), ), // 读取组件存储
            &$data.0, // 实体集合
            &$data.1, // 标记集合
            &mut $ser, // 序列化器
        )
        .unwrap(); // 如果失败则 panic
        )*
    };
}

// 针对 WebAssembly 平台的空实现（因为 wasm 无法访问本地文件系统）
#[cfg(target_arch = "wasm32")]
pub fn save_game(_ecs : &mut World) {
}

// 非 wasm 平台的保存函数实现
#[cfg(not(target_arch = "wasm32"))]
pub fn save_game(ecs : &mut World) {
    // 克隆地图并创建一个临时实体用于保存地图数据
    let mapcopy = ecs.get_mut::<super::map::Map>().unwrap().clone();
    let savehelper = ecs
        .create_entity()
        .with(SerializationHelper{ map : mapcopy }) // 添加地图数据组件
        .marked::<SimpleMarker<SerializeMe>>() // 添加标记组件
        .build();

    // 执行序列化
    {
        let data = ( ecs.entities(), ecs.read_storage::<SimpleMarker<SerializeMe>>() );

        let writer = File::create("./savegame.json").unwrap(); // 创建保存文件
        let mut serializer = serde_json::Serializer::new(writer); // 创建 JSON 序列化器

        // 使用宏序列化所有需要保存的组件
        serialize_individually!(ecs, serializer, data, Position, Renderable, Player, Viewshed, Monster,
            Name, BlocksTile, CombatStats, SufferDamage, WantsToMelee, Item, Consumable, Ranged, InflictsDamage,
            AreaOfEffect, Confusion, ProvidesHealing, InBackpack, WantsToPickupItem, WantsToUseItem,
            WantsToDropItem, SerializationHelper, Equippable, Equipped, MeleePowerBonus, DefenseBonus,
            WantsToRemoveItem, ParticleLifetime, HungerClock, ProvidesFood, MagicMapper
        );
    }

    // 删除临时保存地图的实体，清理现场
    ecs.delete_entity(savehelper).expect("Crash on cleanup");
}

// 检查保存文件是否存在
pub fn does_save_exist() -> bool {
    Path::new("./savegame.json").exists()
}

// 定义一个宏，用于反序列化多个组件类型
macro_rules! deserialize_individually {
    ($ecs:expr, $de:expr, $data:expr, $( $type:ty),*) => {
        $(
        // 对每个组件类型进行反序列化
        DeserializeComponents::<NoError, _>::deserialize(
            &mut ( &mut $ecs.write_storage::<$type>(), ), // 写入组件存储
            &$data.0, // 实体集合
            &mut $data.1, // 标记集合
            &mut $data.2, // 分配器
            &mut $de, // 反序列化器
        )
        .unwrap();
        )*
    };
}

// 加载游戏状态
pub fn load_game(ecs: &mut World) {
    {
        // 删除当前所有实体，清空 ECS
        let mut to_delete = Vec::new();
        for e in ecs.entities().join() {
            to_delete.push(e);
        }
        for del in to_delete.iter() {
            ecs.delete_entity(*del).expect("Deletion failed");
        }
    }

    // 读取保存文件内容
    let data = fs::read_to_string("./savegame.json").unwrap();
    let mut de = serde_json::Deserializer::from_str(&data);

    {
        // 构建反序列化所需的元组：实体集合、标记集合、分配器
        let mut d = (&mut ecs.entities(), &mut ecs.write_storage::<SimpleMarker<SerializeMe>>(), &mut ecs.write_resource::<SimpleMarkerAllocator<SerializeMe>>());

        // 使用宏反序列化所有组件
        deserialize_individually!(ecs, de, d, Position, Renderable, Player, Viewshed, Monster,
            Name, BlocksTile, CombatStats, SufferDamage, WantsToMelee, Item, Consumable, Ranged, InflictsDamage,
            AreaOfEffect, Confusion, ProvidesHealing, InBackpack, WantsToPickupItem, WantsToUseItem,
            WantsToDropItem, SerializationHelper, Equippable, Equipped, MeleePowerBonus, DefenseBonus,
            WantsToRemoveItem, ParticleLifetime, HungerClock, ProvidesFood, MagicMapper
        );
    }

    let mut deleteme : Option<Entity> = None;
    {
        // 恢复地图和玩家位置
        let entities = ecs.entities();
        let helper = ecs.read_storage::<SerializationHelper>();
        let player = ecs.read_storage::<Player>();
        let position = ecs.read_storage::<Position>();

        // 找到保存地图的实体并恢复地图资源
        for (e,h) in (&entities, &helper).join() {
            let mut worldmap = ecs.write_resource::<super::map::Map>();
            *worldmap = h.map.clone(); // 恢复地图数据
            worldmap.tile_content = vec![Vec::new(); super::map::MAPCOUNT]; // 重建 tile_content
            deleteme = Some(e); // 记录该实体以便删除
        }

        // 找到玩家实体并恢复其位置和引用
        for (e,_p,pos) in (&entities, &player, &position).join() {
            let mut ppos = ecs.write_resource::<rltk::Point>();
            *ppos = rltk::Point::new(pos.x, pos.y); // 设置玩家位置
            let mut player_resource = ecs.write_resource::<Entity>();
            *player_resource = e; // 设置玩家实体引用
        }
    }

    // 删除临时地图实体
    ecs.delete_entity(deleteme.unwrap()).expect("Unable to delete helper");
}

// 删除保存文件，实现“永久死亡”机制
pub fn delete_save() {
    if Path::new("./savegame.json").exists() {
        std::fs::remove_file("./savegame.json").expect("Unable to delete file");
    }
}
