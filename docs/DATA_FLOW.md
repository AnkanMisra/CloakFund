# Data Flow

User → Frontend → Rust API → Blockchain → Watcher → Convex Database → UI

---

Payment flow:

generate address  
send funds  
detect deposit (saves to Convex)  
update dashboard (reads from Convex)  
store encrypted receipt
