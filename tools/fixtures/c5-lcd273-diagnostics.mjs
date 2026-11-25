// Synthetic parser/transport fixture only; not an observation of running firmware.
export const c5Lcd273Snapshot=()=>({schemaVersion:1,board:'waveshare-c5-lcd273',counterEncoding:'u64-decimal',state:{
  display:{resetHigh:true,sleepOut:true,displayOn:true,visible:true,brightness:192,pixelFormat:102,madctl:72,
    pixelsWritten:'307200',ramWriteCommands:'3',resetCount:'1'},boardRejectedTransactions:'0',
  sd:{present:true,transport:'spi2-sdhc',card:{spiMode:true,cardState:'Tran',capacityBlocks:'131072',
    nativeCommandCalls:'6',storageBlocksLoaded:'4',storageBlocksWritten:'1'}}},limits:[
  'nativeCommandCalls counts inner SdCard::command calls including failures, excluding SPI wrapper-handled commands; not all wire CMDs.',
  'storageBlocksLoaded counts backing-store loads before reply completion and valid MISO delivery; guest-consumed reads are unobserved.',
  'storageBlocksWritten counts complete backing-store writes; guest response consumption, FAT mount and file flush are unobserved.',
  'CardState and presence do not establish FAT mounting or browser persistence; electrical timing and physical panel response are unverified.',
  'SD counters belong to the card object: CMD0 and power_cycle retain totals; a new attached card starts a new epoch.',
]});
