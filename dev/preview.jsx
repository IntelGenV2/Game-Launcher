// Browser-only fixture. Nothing here calls the native backend or changes user data.
// Run npm run dev, then open /dev/preview.html. Reload to reset the sample library.
import React from "react";
import { createRoot } from "react-dom/client";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";

const names = ["Astral Voyage", "Cinderfall", "Deep Blue", "Echoes of Sol", "Frost Harbor", "Garden Stories", "Hollow Planet", "Into the Wild", "Juniper Valley", "Kingdom of Stars", "Lost Signal", "Moonlight Rally", "Northern Lights", "Orbit Breaker", "Paper Kingdom", "Quiet Horizon", "Riverbound", "Sunset Riders", "Tidal Shift", "Unbroken", "Velvet Sky", "Winter Circuit", "Xenon Drift", "Yellowstone"];
const games = names.map((name, i) => {
  const art = `<svg xmlns="http://www.w3.org/2000/svg" width="400" height="600"><defs><linearGradient id="a" x2="1" y2="1"><stop stop-color="hsl(${i*31},45%,30%)"/><stop offset="1" stop-color="#07101c"/></linearGradient></defs><rect width="400" height="600" fill="url(#a)"/><circle cx="240" cy="190" r="105" fill="none" stroke="#ffffff30" stroke-width="16"/><path d="M0 450L180 230L400 540V600H0" fill="#ffffff13"/><text x="32" y="530" fill="white" font-family="sans-serif" font-size="25">${name.toUpperCase()}</text></svg>`;
  const id = `sample-${i}`, store = ["steam", "epic", "gog"][i%3], missing = i===4;
  return {id,name,store,launchTarget:"sample.exe",installPath:"C:\\Sample Games\\"+name,coverUrl:"data:image/svg+xml,"+encodeURIComponent(art),coverPath:null,coverSource:"Custom",favorite:i%4===0||i<2,pinned:i%4===0||i<2,hidden:false,missing,playtimeMinutes:i%3===0?0:i*45,lastPlayedAt:i%3===0?null:"2026-09-23T19:00:00Z",dateAdded:"2026-09-01T12:00:00Z",steamAppId:null,genre:"Adventure",genres:["Adventure"],tags:i%2===0?["Co-op"]:[],notes:"",developer:"Sample Studio",publisher:"Sample Studio",releaseYear:2026,description:"Sample library data for checking launcher interactions.",logoPath:null,launchArgs:null,workingDir:null,runAsAdmin:false,saveFolder:null,copies:[{id,store,missing,installPath:"C:\\Sample Games\\"+name,playtimeMinutes:i*45}]};
});
games[0].copies.push({id:"alt",store:"epic",missing:false,installPath:"D:\\Sample Games\\Astral Voyage",playtimeMinutes:0});
let settings = {theme:"emerald",sortBy:"name",cardScale:1,showTitles:true,showStoreLabels:true,gridDensity:"normal",coverCorners:"soft",coverShape:"portrait",reduceMotion:false,showTools:true,automaticBackups:true,libraryLayout:"grid"};
const groups = [{id:"group-1",name:"Weekend games",sortOrder:0,createdAt:"2026-09-01",gameIds:["sample-0","sample-2","sample-3"]}];
const states = new Map();
// Expose mock window actions for checking drag hit areas without moving a real window.
window.isTauri = true;
const windowChecks = document.createElement("output");
windowChecks.id = "preview-window-actions";
windowChecks.hidden = true;
document.body.append(windowChecks);
let dragCalls = 0, maximizeCalls = 0;
const showWindowChecks = () => { windowChecks.textContent = JSON.stringify({dragCalls, maximizeCalls}); };
showWindowChecks();
mockWindows("main");
mockIPC(async (cmd, args={}) => {
  const game = games.find(g=>g.id===args.id);
  switch(cmd) {
    case "plugin:window|start_dragging": dragCalls++; showWindowChecks(); return null;
    case "plugin:window|toggle_maximize": maximizeCalls++; showWindowChecks(); return null;
    case "plugin:window|is_maximized": return false;
    case "get_settings": return {...settings};
    case "save_settings": settings={...args.settings}; return null;
    case "list_games": case "rescan_library": return games.filter(g=>!g.hidden).map(g=>({...g}));
    case "list_hidden_games": return games.filter(g=>g.hidden);
    case "list_groups": return groups;
    case "library_stats": return {total:games.length,favorites:games.filter(g=>g.favorite).length,missing:1,totalPlaytimeMinutes:3000};
    case "list_cover_choice_groups": case "suggest_duplicates": case "list_cover_choices": return [];
    case "get_cover_data_urls": return Object.fromEntries(games.map(game => [game.id, game.coverUrl]));
    case "get_logo_data_urls": return {};
    case "get_cover_data_url": case "get_logo_data_url": return null;
    case "get_game": case "fetch_game_metadata": return {...game};
    case "set_pinned": game.pinned=args.pinned; game.favorite=args.pinned; return {...game};
    case "toggle_favorite": game.pinned=!game.pinned; game.favorite=game.pinned; return {...game};
    case "set_hidden": game.hidden=args.hidden; return {...game};
    case "set_preferred_copy": game.preferredCopyId=args.sourceId; return {...game};
    case "launch_statuses": return [...states.values()];
    case "launch_game": {
      const publish = state => { const status={gameId:game.id,sourceId:args.sourceId||game.id,state}; states.set(game.id,status); void emit("launch-status",status); };
      publish("launching"); setTimeout(()=>publish("running"),1500); setTimeout(()=>{game.playtimeMinutes+=1;publish("finished");},8000); return game;
    }
    case "get_game_stats": return {gameId:args.id,totalPlaytimeMinutes:game.playtimeMinutes,sessionCount:0,avgSessionMinutes:0,lastPlayedAt:game.lastPlayedAt,firstPlayedAt:null,dailyPlaytime:[],sessions:[]};
    case "library_overview": return {hoursThisWeek:2,minutesThisWeek:120,mostPlayed:null,streakDays:2,totalPlaytimeMinutes:3000,gamesPlayed:16,yearInReview:{year:2026,totalMinutes:3000,monthly:[],topGames:[]}};
    case "app_data_path": return "Sample library (browser preview)";
    case "plugin:app|version": return "0.3.6-preview";
    case "plugin:dialog|open": return "sample-backup.zip";
    case "preview_backup": return {gameCount:24,artCount:24,createdAt:"2026-09-24T12:00:00Z",sizeBytes:2097152};
    case "restore_backup": return "Sample recovery backup (preview only)";
    case "system_info": return {hostname:"GAMING-PC",username:"Player",os:"Windows 11",osVersion:"24H2",kernel:"26100",architecture:"AMD64",cpu:"Sample 8-core CPU",cpuCores:16,cpuMhz:4200,ramTotalBytes:34359738368,ramAvailableBytes:21474836480,ramUsedBytes:12884901888,gpu:"Sample GPU",display:"2560 × 1440",monitors:2,uptimeSeconds:7200,disks:[{name:"C:",mount:"C:\\",totalBytes:1099511627776,availableBytes:549755813888}],hardware:{processors:[{NumberOfCores:8,L3CacheSize:32768}],memory:[{DeviceLocator:"DIMM A2",Capacity:17179869184,ConfiguredClockSpeed:6000,Manufacturer:"Sample",PartNumber:"DDR5"}],graphics:[{Name:"Sample GPU",DriverVersion:"32.0.15.1000",VideoProcessor:"Sample GPU",CurrentHorizontalResolution:2560,CurrentVerticalResolution:1440,CurrentRefreshRate:165}],storage:[{FriendlyName:"Game SSD",MediaType:4,BusType:17,HealthStatus:0,Size:1099511627776}],board:[{Manufacturer:"Sample",Product:"Gaming Board"}],bios:[{SMBIOSBIOSVersion:"1.20"}],powerPlan:"Balanced"}};
    case "explorer_places": return [];
    case "list_explorer": return {path:"",parent:null,entries:[]};
    case "fetch_covers": case "open_backups": return null;
    default: if(cmd.startsWith("plugin:")) return null;
      throw new Error("Preview does not implement: "+cmd);
  }
}, {shouldMockEvents:true});
const {default:App} = await import("../src/App");
createRoot(document.getElementById("root")).render(<App/>);
