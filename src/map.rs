use rltk::{ RGB, Rltk, RandomNumberGenerator, BaseMap, Algorithm2D, Point };
use super::{Rect};
use std::cmp::{max, min};
use specs::prelude::*;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;

// 地图常量定义
pub const MAPWIDTH : usize = 80;      // 地图宽度（格子数）
pub const MAPHEIGHT : usize = 43;     // 地图高度（格子数）
pub const MAPCOUNT : usize = MAPHEIGHT * MAPWIDTH;  // 地图总格子数

/// 地图瓦片类型枚举
#[derive(PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum TileType {
    Wall,       // 墙壁 - 阻挡通行
    Floor,      // 地板 - 可以通行
    DownStairs  // 向下的楼梯 - 进入下一层
}

/// 地图数据结构
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Map {
    pub tiles : Vec<TileType>,          // 每个格子的类型（墙壁、地板等）
    pub rooms : Vec<Rect>,              // 地图中的所有房间
    pub width : i32,                    // 地图宽度
    pub height : i32,                   // 地图高度
    pub revealed_tiles : Vec<bool>,     // 已探索的格子（玩家曾经看到过）
    pub visible_tiles : Vec<bool>,      // 当前可见的格子（玩家现在能看到）
    pub blocked : Vec<bool>,            // 被阻挡的格子（有实体阻挡通行）
    pub depth : i32,                    // 地图深度（第几层）
    pub bloodstains : HashSet<usize>,   // 血迹位置集合（战斗留下的痕迹）

    // 跳过序列化：因为这是派生数据，每帧都会重建
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub tile_content : Vec<Vec<Entity>> // 每个格子包含的实体列表（空间索引）
}

// Map结构体的实现
impl Map {
    /// 将二维坐标转换为一维数组索引
    pub fn xy_idx(&self, x: i32, y: i32) -> usize {
        (y as usize * self.width as usize) + x as usize
    }

    /// 将房间区域转换为地板
    fn apply_room_to_map(&mut self, room : &Rect) {
        // 遍历房间内的每个格子（不包括边界）
        for y in room.y1 +1 ..= room.y2 {
            for x in room.x1 + 1 ..= room.x2 {
                let idx = self.xy_idx(x, y);
                self.tiles[idx] = TileType::Floor;  // 设置为地板
            }
        }
    }

    /// 创建水平隧道（连接房间的走廊）
    fn apply_horizontal_tunnel(&mut self, x1:i32, x2:i32, y:i32) {
        for x in min(x1,x2) ..= max(x1,x2) {
            let idx = self.xy_idx(x, y);
            // 检查索引有效性后设置为地板
            if idx > 0 && idx < self.width as usize * self.height as usize {
                self.tiles[idx as usize] = TileType::Floor;
            }
        }
    }

    /// 创建垂直隧道（连接房间的走廊）
    fn apply_vertical_tunnel(&mut self, y1:i32, y2:i32, x:i32) {
        for y in min(y1,y2) ..= max(y1,y2) {
            let idx = self.xy_idx(x, y);
            // 检查索引有效性后设置为地板
            if idx > 0 && idx < self.width as usize * self.height as usize {
                self.tiles[idx as usize] = TileType::Floor;
            }
        }
    }

    /// 检查出口是否有效（是否可以通行）
    fn is_exit_valid(&self, x:i32, y:i32) -> bool {
        // 检查边界
        if x < 1 || x > self.width-1 || y < 1 || y > self.height-1 { return false; }
        let idx = self.xy_idx(x, y);
        !self.blocked[idx]  // 检查是否被阻挡
    }

    /// 初始化阻挡状态（墙壁位置设为阻挡）
    pub fn populate_blocked(&mut self) {
        for (i,tile) in self.tiles.iter_mut().enumerate() {
            self.blocked[i] = *tile == TileType::Wall;  // 墙壁位置不可通行
        }
    }

    /// 清空内容索引（为下一帧的重建做准备）
    pub fn clear_content_index(&mut self) {
        for content in self.tile_content.iter_mut() {
            content.clear();  // 清空每个格子的实体列表
        }
    }

    /// 创建新的随机地图（房间和走廊布局）
    pub fn new_map_rooms_and_corridors(new_depth : i32) -> Map {
        // 初始化地图，所有格子默认为墙壁
        let mut map = Map{
            tiles : vec![TileType::Wall; MAPCOUNT],
            rooms : Vec::new(),
            width : MAPWIDTH as i32,
            height: MAPHEIGHT as i32,
            revealed_tiles : vec![false; MAPCOUNT],  // 初始都未探索
            visible_tiles : vec![false; MAPCOUNT],   // 初始都不可见
            blocked : vec![false; MAPCOUNT],         // 初始都未阻挡
            tile_content : vec![Vec::new(); MAPCOUNT], // 初始化空的内容索引
            depth: new_depth,                        // 设置地图层数
            bloodstains: HashSet::new()              // 初始化空的血迹集合
        };

        // 地图生成参数
        const MAX_ROOMS : i32 = 30;   // 最大尝试生成房间数
        const MIN_SIZE : i32 = 6;     // 房间最小尺寸
        const MAX_SIZE : i32 = 10;    // 房间最大尺寸

        let mut rng = RandomNumberGenerator::new();  // 随机数生成器

        // 尝试生成多个房间
        for _i in 0..MAX_ROOMS {
            // 随机生成房间尺寸和位置
            let w = rng.range(MIN_SIZE, MAX_SIZE);
            let h = rng.range(MIN_SIZE, MAX_SIZE);
            let x = rng.roll_dice(1, map.width - w - 1) - 1;
            let y = rng.roll_dice(1, map.height - h - 1) - 1;
            let new_room = Rect::new(x, y, w, h);
            
            // 检查新房间是否与现有房间重叠
            let mut ok = true;
            for other_room in map.rooms.iter() {
                if new_room.intersect(other_room) { ok = false }
            }
            
            // 如果不重叠，则添加到地图
            if ok {
                map.apply_room_to_map(&new_room);  // 将房间区域设为地板

                // 如果不是第一个房间，创建走廊连接
                if !map.rooms.is_empty() {
                    let (new_x, new_y) = new_room.center();      // 新房间中心
                    let (prev_x, prev_y) = map.rooms[map.rooms.len()-1].center();  // 前一个房间中心
                    
                    // 随机选择走廊走向（先横后竖 或 先竖后横）
                    if rng.range(0,2) == 1 {
                        map.apply_horizontal_tunnel(prev_x, new_x, prev_y);  // 水平隧道
                        map.apply_vertical_tunnel(prev_y, new_y, new_x);     // 垂直隧道
                    } else {
                        map.apply_vertical_tunnel(prev_y, new_y, prev_x);    // 垂直隧道
                        map.apply_horizontal_tunnel(prev_x, new_x, new_y);   // 水平隧道
                    }
                }

                map.rooms.push(new_room);  // 将房间添加到房间列表
            }
        }

        // 在最后一个房间中心放置向下的楼梯
        let stairs_position = map.rooms[map.rooms.len()-1].center();
        let stairs_idx = map.xy_idx(stairs_position.0, stairs_position.1);
        map.tiles[stairs_idx] = TileType::DownStairs;

        map  // 返回生成的地图
    }
}

/// 实现 BaseMap trait（路径寻找需要）
impl BaseMap for Map {
    /// 检查格子是否不透明（墙壁不透明，其他透明）
    fn is_opaque(&self, idx:usize) -> bool {
        self.tiles[idx] == TileType::Wall
    }

    /// 计算两个格子之间的路径距离（用于A*寻路）
    fn get_pathing_distance(&self, idx1:usize, idx2:usize) -> f32 {
        let w = self.width as usize;
        let p1 = Point::new(idx1 % w, idx1 / w);  // 将索引转换为坐标
        let p2 = Point::new(idx2 % w, idx2 / w);
        rltk::DistanceAlg::Pythagoras.distance2d(p1, p2)  // 计算欧几里得距离
    }

    /// 获取可用的出口（相邻的可通行格子）
    fn get_available_exits(&self, idx:usize) -> rltk::SmallVec<[(usize, f32); 10]> {
        let mut exits = rltk::SmallVec::new();  // 使用小向量优化
        let x = idx as i32 % self.width;        // 当前格子的x坐标
        let y = idx as i32 / self.width;        // 当前格子的y坐标
        let w = self.width as usize;            // 地图宽度

        // 检查四个基本方向（上下左右）
        if self.is_exit_valid(x-1, y) { exits.push((idx-1, 1.0)) };      // 左
        if self.is_exit_valid(x+1, y) { exits.push((idx+1, 1.0)) };      // 右
        if self.is_exit_valid(x, y-1) { exits.push((idx-w, 1.0)) };      // 上
        if self.is_exit_valid(x, y+1) { exits.push((idx+w, 1.0)) };      // 下

        // 检查四个对角线方向（移动成本更高）
        if self.is_exit_valid(x-1, y-1) { exits.push(((idx-w)-1, 1.45)); }  // 左上
        if self.is_exit_valid(x+1, y-1) { exits.push(((idx-w)+1, 1.45)); }  // 右上
        if self.is_exit_valid(x-1, y+1) { exits.push(((idx+w)-1, 1.45)); }  // 左下
        if self.is_exit_valid(x+1, y+1) { exits.push(((idx+w)+1, 1.45)); }  // 右下

        exits  // 返回可用出口列表
    }
}

/// 实现 Algorithm2D trait（二维算法需要）
impl Algorithm2D for Map {
    /// 返回地图尺寸
    fn dimensions(&self) -> Point {
        Point::new(self.width, self.height)
    }
}

/// 检查指定位置是否是已探索的墙壁
fn is_revealed_and_wall(map: &Map, x: i32, y: i32) -> bool {
    let idx = map.xy_idx(x, y);
    map.tiles[idx] == TileType::Wall && map.revealed_tiles[idx]  // 是墙壁且已探索
}

/// 根据周围墙壁情况选择合适的墙壁字符（连接墙壁的显示）
fn wall_glyph(map : &Map, x: i32, y:i32) -> rltk::FontCharType {
    // 边界检查，如果是边界返回普通墙壁字符
    if x < 1 || x > map.width-2 || y < 1 || y > map.height-2 as i32 { return 35; }
    
    let mut mask : u8 = 0;  // 使用位掩码表示周围墙壁情况

    // 检查四个方向的邻居墙壁（使用位掩码记录）
    if is_revealed_and_wall(map, x, y - 1) { mask +=1; }   // 上 (位0)
    if is_revealed_and_wall(map, x, y + 1) { mask +=2; }   // 下 (位1) 
    if is_revealed_and_wall(map, x - 1, y) { mask +=4; }   // 左 (位2)
    if is_revealed_and_wall(map, x + 1, y) { mask +=8; }   // 右 (位3)

    // 根据周围墙壁情况返回合适的Box-drawing字符
    match mask {
        0 => { 9 }      // 9: 孤立墙壁（看不到邻居）
        1 => { 186 }    // │ 只有上方有墙壁
        2 => { 186 }    // │ 只有下方有墙壁  
        3 => { 186 }    // │ 上下都有墙壁
        4 => { 205 }    // ─ 只有左边有墙壁
        5 => { 188 }    // ┘ 左上角
        6 => { 187 }    // └ 左下角
        7 => { 185 }    // ┴ 上左右三面墙
        8 => { 205 }    // ─ 只有右边有墙壁
        9 => { 200 }    // ┐ 右上角
        10 => { 201 }   // ┌ 右下角
        11 => { 204 }   // ├ 左右下三面墙
        12 => { 205 }   // ─ 左右都有墙壁
        13 => { 202 }   // ┬ 上右下三面墙
        14 => { 203 }   // ┤ 上左下三面墙
        15 => { 206 }   // ╬ 四面都有墙壁
        _ => { 35 }     // # 默认墙壁字符（不应该到达这里）
    }
}

/// 绘制地图到屏幕
pub fn draw_map(ecs: &World, ctx : &mut Rltk) {
    let map = ecs.fetch::<Map>();  // 从ECS获取地图资源

    let mut y = 0;
    let mut x = 0;
    
    // 遍历所有地图格子
    for (idx,tile) in map.tiles.iter().enumerate() {
        // 只渲染已探索的格子
        if map.revealed_tiles[idx] {
            let glyph;    // 要显示的字符
            let mut fg;   // 前景色
            let mut bg = RGB::from_f32(0., 0., 0.);  // 背景色（默认黑色）

            // 根据瓦片类型设置字符和颜色
            match tile {
                TileType::Floor => {
                    glyph = rltk::to_cp437('.');          // 地板用点表示
                    fg = RGB::from_f32(0.0, 0.5, 0.5);   // 青绿色
                }
                TileType::Wall => {
                    glyph = wall_glyph(&*map, x, y);      // 使用智能墙壁字符
                    fg = RGB::from_f32(0., 1.0, 0.);      // 绿色
                }
                TileType::DownStairs => {
                    glyph = rltk::to_cp437('>');          // 楼梯用>表示
                    fg = RGB::from_f32(0., 1.0, 1.0);     // 青色
                }
            }
            
            // 如果有血迹，设置红色背景
            if map.bloodstains.contains(&idx) { 
                bg = RGB::from_f32(0.75, 0., 0.);  // 深红色背景
            }
            
            // 如果当前不可见，使用灰度显示
            if !map.visible_tiles[idx] {
                fg = fg.to_greyscale();           // 前景色变灰
                bg = RGB::from_f32(0., 0., 0.);   // 背景色变黑（不显示血迹）
            }
            
            // 在屏幕上绘制格子
            ctx.set(x, y, fg, bg, glyph);
        }

        // 移动坐标到下一个格子
        x += 1;
        if x > MAPWIDTH as i32-1 {  // 如果到达行尾
            x = 0;                  // 回到行首
            y += 1;                 // 换到下一行
        }
    }
}