## server.rs 流程
### 一、从「进程启动」开始（没有任何网络）

假设现在有 两个节点：

```nginx
Node B（中心节点） 127.0.0.1:2001
Node A（新节点）   127.0.0.1:3001
```

两者代码完全相同。

#### 1️⃣ Node B 先启动
```rust
Server::run("127.0.0.1:2001")
```

CPU 做的事情：
- TcpListener::bind("127.0.0.1:2001")
- 进入 for stream in listener.incoming() 阻塞等待

此时：
- Node B 什么都没发
- 只是“坐着等别人连”

#### 2️⃣ Node A 启动
```rust
Server::run("127.0.0.1:3001")
```

CPU 顺序执行：

1、监听端口
```rust
TcpListener::bind("127.0.0.1:3001")
```

Node A 也进入监听状态。

2、关键一步：主动发 Version
```rust
if addr != CENTERAL_NODE {
    send_version(CENTERAL_NODE, best_height);
}
```

**这是整个流程的起点。**

Node A 主动发：
```rust
Version {
  addr_from: "127.0.0.1:3001",
  version: 1,
  best_height: 0
}
```

通过：
```rust
TcpStream::connect("127.0.0.1:2001")
```

### 二、Node B 收到 Version（serve() 第一次被触发）

Node B 的 `listener.incoming()` 收到连接：
```rust
thread::spawn(|| serve(blockchain, stream))
```

进入：
```rust
serve(blockchain, stream)
```

#### 3️⃣ Node B 处理 Version 包
```rust
match pkg {
    Package::Version { addr_from, version, best_height } => {
        ...
    }
}
```

此时：

```nginx
addr_from   = "127.0.0.1:3001"
best_height = 0
```

Node B 的本地高度：
```rust
local_best_height = 100
```

1、判断高度
```rust
if local_best_height > best_height {
    send_version(addr_from, local_best_height);
}
```

Node B 主动回一个 Version：
```rust
Version {
  addr_from: "127.0.0.1:2001",
  best_height: 100
}
```

2、记录新节点
```rust
GLOBAL_NODES.add_node(addr_from)
```

### 三、Node A 收到 Version（serve() 第二次被触发）

Node A 收到来自 Node B 的 Version：
```nginx
best_height = 100
```

Node A 的本地高度：
```nginx
local_best_height = 0
```

#### 4️⃣ Node A 判断：我落后了
```rust
if local_best_height < best_height {
    send_get_blocks(addr_from);
}
```

这一步非常重要：

> 这是第一次真正“请求区块”的动作

Node A 发：
```rust
GetBlocks { addr_from: "127.0.0.1:3001" }
```

### 四、Node B 收到 GetBlocks

Node B 的 serve() 再次被触发。

#### 5️⃣ Node B 处理 GetBlocks
```rust
Package::GetBlocks { addr_from } => {
    let blocks = blockchain.get_block_hashes();
    send_inv(addr_from, OpType::Block, &blocks);
}
```

CPU 做的事情：
- 遍历本地区块链
- 拿到 [h1, h2, h3, ..., h100]
- 发送：
```rust
Inv {
  op_type: Block,
  items: [h1, h2, h3, ..., h100]
}
```

### 五、Node A 收到 Inv（这是关键转折）

Node A 收到：
```rust
Inv(Block, [h1, h2, ..., h100])
```

#### 6️⃣ Node A 处理 Inv(Block)
```rust
Package::Inv { op_type: Block, items } => {
    GLOBAL_BLOCKS_IN_TRANSIT.add_blocks(items);
    let block_hash = items.get(0).unwrap();
    send_get_data(addr_from, OpType::Block, block_hash);
    GLOBAL_BLOCKS_IN_TRANSIT.remove(block_hash);
}
```

CPU 真实做的事：

1、记录“我接下来要同步这些区块”
```scss
GLOBAL_BLOCKS_IN_TRANSIT = [h1, h2, h3, ..., h100]
```

2、请求第一个区块
```rust
GetData(Block, h1)
```

### 六、Node B 收到 GetData(Block)

Node B 进入：
```rust
Package::GetData { op_type: Block, id } => {
    let block = blockchain.get_block(id);
    send_block(addr_from, &block);
}
```

Node B 发：
```rust
Block { block: serialize(h1) }
```

### 七、Node A 收到 Block（进入循环）

Node A 执行：
```rust
Package::Block { block } => {
    blockchain.add_block(block);

    if GLOBAL_BLOCKS_IN_TRANSIT.len() > 0 {
        let next = GLOBAL_BLOCKS_IN_TRANSIT.first();
        send_get_data(Block, next);
        GLOBAL_BLOCKS_IN_TRANSIT.remove(next);
    }
}
```

CPU 顺序：
- 写入区块 h1
- 发现还有 h2 ~ h100
- 请求 h2

### 🔁 这个过程会重复 99 次
```scss
GetData(h2) → Block(h2)
GetData(h3) → Block(h3)
...
GetData(h100) → Block(h100)
```

### 八、Node A 同步完成（真正的终点）

当最后一个 block 被处理完：
```rust
GLOBAL_BLOCKS_IN_TRANSIT.len() == 0
```

于是执行：
```rust
utxo_set.reindex();
```

这一步意味着：

“我的区块链现在和网络主流一致了”

### 九、此时网络进入“稳态”

之后只发生：
- 新交易 → Tx / Inv
- 新区块 → Inv(Block)
- 有需要才 GetData

### 十、你真正该记住的 3 件事
1️⃣ 一切始于 send_version
没有它，网络不会动。

2️⃣ 所有“请求”都是条件触发的
没有定时器，没有轮询：
```nginx
Version → GetBlocks
Inv     → GetData
```

3️⃣ serve() 不是“处理请求”，而是“推进状态”

它做的不是：
> “给我一个请求，我给你一个响应”

而是：
> “根据当前状态，决定下一步该发什么包”

## 状态机

### 一、先给结论：你的节点只有 5 个核心状态

节点不是“服务端 / 客户端”，而是一个有限状态机（FSM）
```rust
enum NodeState {
    Booting,        // 刚启动
    Handshaking,    // Version 交换中
    SyncingBlocks,  // 正在同步区块
    Synced,         // 区块链已同步
    Running,        // 正常运行（交易/挖矿/广播）
}
```

实际上 `Synced` 和 `Running` 可以合并，但我分开是为了你更好理解。

### 二、每个状态在“干什么”（非常关键）
#### 1️⃣ Booting（启动态）

进入条件：进程刚启动

只做一件事
```rust
send_version(CENTERAL_NODE, best_height)
```

立刻进入
```nginx
Booting → Handshaking
```

#### 2️⃣ Handshaking（版本协商）

能收到的包
```text
Version
```

逻辑
```rust
if local_height < remote_height {
    send_get_blocks(peer)
}
if local_height > remote_height {
    send_version(peer)
}
```

状态转移
```nginx
Handshaking
  ├─ 我落后 → SyncingBlocks
  └─ 我不落后 → Synced
```

#### 3️⃣ SyncingBlocks（区块同步中）

这是最重要的状态。

这个状态的“内部变量”
```rust
GLOBAL_BLOCKS_IN_TRANSIT = [h1, h2, h3, ...]
```

能收到的包 & 行为
收到 `Inv(Block, hashes)`
```rust
add_blocks(hashes)
send_get_data(h1)
```

状态：不变

收到 Block
```rust
add_block(block)

if has_next_block {
    send_get_data(next_hash)
} else {
    utxo_set.reindex()
}
```

状态转移
```nginx
SyncingBlocks → Synced
```

#### 4️⃣ Synced（刚同步完成）

进入条件
```rust
GLOBAL_BLOCKS_IN_TRANSIT.len() == 0
```

动作
```rust
utxo_set.reindex()
```

立刻进入
```rust
Synced → Running
```

#### 5️⃣ Running（正常运行态）

这是最长时间停留的状态。

能处理的消息
| 消息         | 行为                  |
| ---------- | ------------------- |
| Tx         | 放入 mempool / 广播     |
| Inv(Tx)    | GetData(Tx)         |
| Inv(Block) | GetData(Block)      |
| Block      | add_block + reindex |
| Version    | 可能重新触发 GetBlocks    |


### 三、完整状态转移表（核心）

> 这张表就是你整个网络协议的“真相”

| 当前状态          | 收到 Package | 条件    | 动作                   | 下一个状态         |
| ------------- | ---------- | ----- | -------------------- | ------------- |
| Booting       | —          | —     | send_version         | Handshaking   |
| Handshaking   | Version    | 我落后   | send_get_blocks      | SyncingBlocks |
| Handshaking   | Version    | 不落后   | —                    | Synced        |
| SyncingBlocks | Inv(Block) | —     | add_blocks + GetData | SyncingBlocks |
| SyncingBlocks | Block      | 还有未同步 | GetData(next)        | SyncingBlocks |
| SyncingBlocks | Block      | 同步完   | reindex              | Synced        |
| Synced        | —          | —     | —                    | Running       |
| Running       | Tx         | —     | mempool + inv        | Running       |
| Running       | Inv(Block) | —     | GetData(Block)       | SyncingBlocks |
| Running       | Version    | 对方更高  | GetBlocks            | SyncingBlocks |

### 四、用状态机重读你最困惑的代码（关键）
你之前最迷糊的地方
```rust
Package::Inv { op_type: Block, items } => {
    GLOBAL_BLOCKS_IN_TRANSIT.add_blocks(items);
    let block_hash = items.get(0).unwrap();
    send_get_data(addr_from, OpType::Block, block_hash);
    GLOBAL_BLOCKS_IN_TRANSIT.remove(block_hash);
}
```

状态机视角解读

- 前提状态：`Handshaking` 或 `Running`
- 事件：收到区块目录
- 动作：
    - 进入 SyncingBlocks
    - 启动区块拉取
- 这不是“响应请求”
- 这是“状态跃迁的触发器”

再看 `Package::Block`
```rust
Package::Block => {
    add_block();

    if in_transit_not_empty {
        GetData(next)
    } else {
        reindex()
    }
}
```

状态机解读：
```scss
SyncingBlocks
   ↓
（Block）
   ↓
SyncingBlocks 或 Synced
```

### 五、为什么这个模型不会死循环？

因为所有主动发送的包，都是状态驱动的：
- Version 只在启动 / 高度变化时发
- GetBlocks 只在发现自己落后时发
- GetData 只在 Inv 或 SyncingBlocks 时发
- Inv 只在“我有你可能没有的东西”时发

> 没有定时器，没有 while true 网络请求
