import type { EmojiItem } from "../../types";
import { fetchEmojis } from "../../api";
import { $, h } from "../../utils/dom";

export const EmojiPicker = {
  isOpen: false,
  category: "all",
  searchQuery: "",
  textarea: null as HTMLTextAreaElement | null,
  allEmojis: [] as EmojiItem[],
  onInsert: null as ((emoji: string) => void) | null,

  data: {
    smileys: [
      { emoji: "😀", name: "grinning face", shortcode: "grinning" },
      { emoji: "😃", name: "grinning face with big eyes", shortcode: "smiley" },
      { emoji: "😄", name: "grinning face with smiling eyes", shortcode: "smile" },
      { emoji: "😁", name: "beaming face with smiling eyes", shortcode: "grin" },
      { emoji: "😆", name: "grinning squinting face", shortcode: "laughing" },
      { emoji: "😅", name: "grinning face with sweat", shortcode: "sweat_smile" },
      { emoji: "🤣", name: "rolling on the floor laughing", shortcode: "rofl" },
      { emoji: "😂", name: "face with tears of joy", shortcode: "joy" },
      { emoji: "🙂", name: "slightly smiling face", shortcode: "slightly_smiling_face" },
      { emoji: "🙃", name: "upside-down face", shortcode: "upside_down_face" },
      { emoji: "😉", name: "winking face", shortcode: "wink" },
      { emoji: "😊", name: "smiling face with smiling eyes", shortcode: "blush" },
      { emoji: "😇", name: "smiling face with halo", shortcode: "innocent" },
      { emoji: "🥰", name: "smiling face with hearts", shortcode: "smiling_face_with_three_hearts" },
      { emoji: "😍", name: "smiling face with heart-eyes", shortcode: "heart_eyes" },
      { emoji: "🤩", name: "star-struck", shortcode: "star_struck" },
      { emoji: "😘", name: "face blowing a kiss", shortcode: "kissing_heart" },
      { emoji: "😋", name: "face savoring food", shortcode: "yum" },
      { emoji: "😛", name: "face with tongue", shortcode: "stuck_out_tongue" },
      { emoji: "😜", name: "winking face with tongue", shortcode: "stuck_out_tongue_winking_eye" },
      { emoji: "🤪", name: "zany face", shortcode: "zany_face" },
      { emoji: "🤑", name: "money-mouth face", shortcode: "money_mouth_face" },
      { emoji: "🤗", name: "smiling face with open hands", shortcode: "hugs" },
      { emoji: "🤔", name: "thinking face", shortcode: "thinking" },
      { emoji: "🤐", name: "zipper-mouth face", shortcode: "zipper_mouth_face" },
      { emoji: "😐", name: "neutral face", shortcode: "neutral_face" },
      { emoji: "😏", name: "smirking face", shortcode: "smirk" },
      { emoji: "😒", name: "unamused face", shortcode: "unamused" },
      { emoji: "🙄", name: "face with rolling eyes", shortcode: "roll_eyes" },
      { emoji: "😬", name: "grimacing face", shortcode: "grimacing" },
      { emoji: "😌", name: "relieved face", shortcode: "relieved" },
      { emoji: "😴", name: "sleeping face", shortcode: "sleeping" },
      { emoji: "😷", name: "face with medical mask", shortcode: "mask" },
      { emoji: "🤢", name: "nauseated face", shortcode: "nauseated_face" },
      { emoji: "🤮", name: "face vomiting", shortcode: "vomiting_face" },
      { emoji: "🥵", name: "hot face", shortcode: "hot_face" },
      { emoji: "🥶", name: "cold face", shortcode: "cold_face" },
      { emoji: "🥴", name: "woozy face", shortcode: "woozy_face" },
      { emoji: "🤯", name: "exploding head", shortcode: "exploding_head" },
      { emoji: "🥳", name: "partying face", shortcode: "partying_face" },
      { emoji: "😎", name: "smiling face with sunglasses", shortcode: "sunglasses" },
      { emoji: "🤓", name: "nerd face", shortcode: "nerd_face" },
      { emoji: "🧐", name: "face with monocle", shortcode: "monocle_face" },
      { emoji: "🥺", name: "pleading face", shortcode: "pleading_face" },
      { emoji: "😢", name: "crying face", shortcode: "cry" },
      { emoji: "😭", name: "loudly crying face", shortcode: "sob" },
      { emoji: "😱", name: "face screaming in fear", shortcode: "scream" },
      { emoji: "😤", name: "face with steam from nose", shortcode: "triumph" },
      { emoji: "😡", name: "enraged face", shortcode: "rage" },
      { emoji: "😠", name: "angry face", shortcode: "angry" },
      { emoji: "🤬", name: "face with symbols on mouth", shortcode: "cursing_face" },
      { emoji: "💀", name: "skull", shortcode: "skull" },
      { emoji: "💩", name: "pile of poo", shortcode: "poop" },
      { emoji: "🤡", name: "clown face", shortcode: "clown_face" },
      { emoji: "👻", name: "ghost", shortcode: "ghost" },
      { emoji: "👽", name: "alien", shortcode: "alien" },
      { emoji: "🤖", name: "robot", shortcode: "robot" },
    ] as EmojiItem[],
    people: [
      { emoji: "👋", name: "waving hand", shortcode: "wave" },
      { emoji: "✋", name: "raised hand", shortcode: "hand" },
      { emoji: "👌", name: "OK hand", shortcode: "ok_hand" },
      { emoji: "✌️", name: "victory hand", shortcode: "v" },
      { emoji: "🤞", name: "crossed fingers", shortcode: "crossed_fingers" },
      { emoji: "🤟", name: "love-you gesture", shortcode: "love_you_gesture" },
      { emoji: "🤘", name: "sign of the horns", shortcode: "metal" },
      { emoji: "🤙", name: "call me hand", shortcode: "call_me_hand" },
      { emoji: "👈", name: "backhand index pointing left", shortcode: "point_left" },
      { emoji: "👉", name: "backhand index pointing right", shortcode: "point_right" },
      { emoji: "👆", name: "backhand index pointing up", shortcode: "point_up_2" },
      { emoji: "👇", name: "backhand index pointing down", shortcode: "point_down" },
      { emoji: "👍", name: "thumbs up", shortcode: "+1" },
      { emoji: "👎", name: "thumbs down", shortcode: "-1" },
      { emoji: "✊", name: "raised fist", shortcode: "fist" },
      { emoji: "👊", name: "oncoming fist", shortcode: "facepunch" },
      { emoji: "👏", name: "clapping hands", shortcode: "clap" },
      { emoji: "🙌", name: "raising hands", shortcode: "raised_hands" },
      { emoji: "👐", name: "open hands", shortcode: "open_hands" },
      { emoji: "🤝", name: "handshake", shortcode: "handshake" },
      { emoji: "🙏", name: "folded hands", shortcode: "pray" },
      { emoji: "✍️", name: "writing hand", shortcode: "writing_hand" },
      { emoji: "💅", name: "nail polish", shortcode: "nail_care" },
      { emoji: "🤳", name: "selfie", shortcode: "selfie" },
      { emoji: "💪", name: "flexed biceps", shortcode: "muscle" },
      { emoji: "👀", name: "eyes", shortcode: "eyes" },
      { emoji: "🧠", name: "brain", shortcode: "brain" },
      { emoji: "👶", name: "baby", shortcode: "baby" },
      { emoji: "🧑", name: "person", shortcode: "adult" },
      { emoji: "👨", name: "man", shortcode: "man" },
      { emoji: "👩", name: "woman", shortcode: "woman" },
    ] as EmojiItem[],
    nature: [
      { emoji: "🐶", name: "dog face", shortcode: "dog" },
      { emoji: "🐱", name: "cat face", shortcode: "cat" },
      { emoji: "🐭", name: "mouse face", shortcode: "mouse" },
      { emoji: "🐹", name: "hamster", shortcode: "hamster" },
      { emoji: "🐰", name: "rabbit face", shortcode: "rabbit" },
      { emoji: "🦊", name: "fox", shortcode: "fox_face" },
      { emoji: "🐻", name: "bear", shortcode: "bear" },
      { emoji: "🐼", name: "panda", shortcode: "panda_face" },
      { emoji: "🐨", name: "koala", shortcode: "koala" },
      { emoji: "🐯", name: "tiger face", shortcode: "tiger" },
      { emoji: "🦁", name: "lion", shortcode: "lion" },
      { emoji: "🐮", name: "cow face", shortcode: "cow" },
      { emoji: "🐷", name: "pig face", shortcode: "pig" },
      { emoji: "🐸", name: "frog", shortcode: "frog" },
      { emoji: "🐵", name: "monkey face", shortcode: "monkey_face" },
      { emoji: "🐔", name: "chicken", shortcode: "chicken" },
      { emoji: "🐧", name: "penguin", shortcode: "penguin" },
      { emoji: "🐦", name: "bird", shortcode: "bird" },
      { emoji: "🦄", name: "unicorn", shortcode: "unicorn" },
      { emoji: "🐝", name: "honeybee", shortcode: "bee" },
      { emoji: "🦋", name: "butterfly", shortcode: "butterfly" },
      { emoji: "🐙", name: "octopus", shortcode: "octopus" },
      { emoji: "🐬", name: "dolphin", shortcode: "dolphin" },
      { emoji: "🐳", name: "spouting whale", shortcode: "whale" },
      { emoji: "🦈", name: "shark", shortcode: "shark" },
      { emoji: "🌲", name: "evergreen tree", shortcode: "evergreen_tree" },
      { emoji: "🌳", name: "deciduous tree", shortcode: "deciduous_tree" },
      { emoji: "🌴", name: "palm tree", shortcode: "palm_tree" },
      { emoji: "🌱", name: "seedling", shortcode: "seedling" },
      { emoji: "🌿", name: "herb", shortcode: "herb" },
      { emoji: "🍀", name: "four leaf clover", shortcode: "four_leaf_clover" },
      { emoji: "🌸", name: "cherry blossom", shortcode: "cherry_blossom" },
      { emoji: "🌹", name: "rose", shortcode: "rose" },
      { emoji: "🌻", name: "sunflower", shortcode: "sunflower" },
      { emoji: "🌞", name: "sun with face", shortcode: "sun_with_face" },
      { emoji: "⭐", name: "star", shortcode: "star" },
      { emoji: "🌈", name: "rainbow", shortcode: "rainbow" },
      { emoji: "⚡", name: "high voltage", shortcode: "zap" },
      { emoji: "❄️", name: "snowflake", shortcode: "snowflake" },
      { emoji: "🔥", name: "fire", shortcode: "fire" },
    ] as EmojiItem[],
    food: [
      { emoji: "🍏", name: "green apple", shortcode: "green_apple" },
      { emoji: "🍎", name: "red apple", shortcode: "apple" },
      { emoji: "🍌", name: "banana", shortcode: "banana" },
      { emoji: "🍉", name: "watermelon", shortcode: "watermelon" },
      { emoji: "🍇", name: "grapes", shortcode: "grapes" },
      { emoji: "🍓", name: "strawberry", shortcode: "strawberry" },
      { emoji: "🍒", name: "cherries", shortcode: "cherries" },
      { emoji: "🍑", name: "peach", shortcode: "peach" },
      { emoji: "🍍", name: "pineapple", shortcode: "pineapple" },
      { emoji: "🥑", name: "avocado", shortcode: "avocado" },
      { emoji: "🌽", name: "ear of corn", shortcode: "corn" },
      { emoji: "🥕", name: "carrot", shortcode: "carrot" },
      { emoji: "🍞", name: "bread", shortcode: "bread" },
      { emoji: "🥐", name: "croissant", shortcode: "croissant" },
      { emoji: "🧀", name: "cheese wedge", shortcode: "cheese" },
      { emoji: "🍖", name: "meat on bone", shortcode: "meat_on_bone" },
      { emoji: "🍗", name: "poultry leg", shortcode: "poultry_leg" },
      { emoji: "🍔", name: "hamburger", shortcode: "hamburger" },
      { emoji: "🍟", name: "french fries", shortcode: "fries" },
      { emoji: "🍕", name: "pizza", shortcode: "pizza" },
      { emoji: "🌭", name: "hot dog", shortcode: "hotdog" },
      { emoji: "🥪", name: "sandwich", shortcode: "sandwich" },
      { emoji: "🌮", name: "taco", shortcode: "taco" },
      { emoji: "🍜", name: "steaming bowl", shortcode: "ramen" },
      { emoji: "🍣", name: "sushi", shortcode: "sushi" },
      { emoji: "🍦", name: "soft ice cream", shortcode: "ice_cream" },
      { emoji: "🎂", name: "birthday cake", shortcode: "birthday" },
      { emoji: "🍫", name: "chocolate bar", shortcode: "chocolate_bar" },
      { emoji: "☕", name: "hot beverage", shortcode: "coffee" },
      { emoji: "🍺", name: "beer mug", shortcode: "beer" },
      { emoji: "🍻", name: "clinking beer mugs", shortcode: "beers" },
      { emoji: "🥂", name: "clinking glasses", shortcode: "clinking_glasses" },
      { emoji: "🍷", name: "wine glass", shortcode: "wine_glass" },
    ] as EmojiItem[],
    travel: [
      { emoji: "🚗", name: "automobile", shortcode: "car" },
      { emoji: "🚕", name: "taxi", shortcode: "taxi" },
      { emoji: "🚌", name: "bus", shortcode: "bus" },
      { emoji: "🏎️", name: "racing car", shortcode: "racing_car" },
      { emoji: "🚓", name: "police car", shortcode: "police_car" },
      { emoji: "🚑", name: "ambulance", shortcode: "ambulance" },
      { emoji: "🚒", name: "fire engine", shortcode: "fire_engine" },
      { emoji: "🏍️", name: "motorcycle", shortcode: "motorcycle" },
      { emoji: "🚲", name: "bicycle", shortcode: "bike" },
      { emoji: "🚆", name: "train", shortcode: "train2" },
      { emoji: "✈️", name: "airplane", shortcode: "airplane" },
      { emoji: "🚀", name: "rocket", shortcode: "rocket" },
      { emoji: "🛸", name: "flying saucer", shortcode: "flying_saucer" },
      { emoji: "⛵", name: "sailboat", shortcode: "boat" },
      { emoji: "🚢", name: "ship", shortcode: "ship" },
      { emoji: "🏠", name: "house", shortcode: "house" },
      { emoji: "🏢", name: "office building", shortcode: "office" },
      { emoji: "🏰", name: "castle", shortcode: "castle" },
    ] as EmojiItem[],
    activities: [
      { emoji: "⚽", name: "soccer ball", shortcode: "soccer" },
      { emoji: "🏀", name: "basketball", shortcode: "basketball" },
      { emoji: "🏈", name: "american football", shortcode: "football" },
      { emoji: "⚾", name: "baseball", shortcode: "baseball" },
      { emoji: "🎾", name: "tennis", shortcode: "tennis" },
      { emoji: "🏐", name: "volleyball", shortcode: "volleyball" },
      { emoji: "🏓", name: "ping pong", shortcode: "ping_pong" },
      { emoji: "🥊", name: "boxing glove", shortcode: "boxing_glove" },
      { emoji: "🎯", name: "direct hit", shortcode: "dart" },
      { emoji: "🏆", name: "trophy", shortcode: "trophy" },
      { emoji: "🥇", name: "1st place medal", shortcode: "first_place_medal" },
      { emoji: "🥈", name: "2nd place medal", shortcode: "second_place_medal" },
      { emoji: "🥉", name: "3rd place medal", shortcode: "third_place_medal" },
      { emoji: "🎪", name: "circus tent", shortcode: "circus_tent" },
      { emoji: "🎨", name: "artist palette", shortcode: "art" },
      { emoji: "🎬", name: "clapper board", shortcode: "clapper" },
      { emoji: "🎤", name: "microphone", shortcode: "microphone" },
      { emoji: "🎧", name: "headphone", shortcode: "headphones" },
      { emoji: "🎸", name: "guitar", shortcode: "guitar" },
      { emoji: "🎮", name: "video game", shortcode: "video_game" },
    ] as EmojiItem[],
    objects: [
      { emoji: "🎉", name: "party popper", shortcode: "tada" },
      { emoji: "🎊", name: "confetti ball", shortcode: "confetti_ball" },
      { emoji: "🎈", name: "balloon", shortcode: "balloon" },
      { emoji: "🎁", name: "wrapped gift", shortcode: "gift" },
      { emoji: "📱", name: "mobile phone", shortcode: "iphone" },
      { emoji: "💻", name: "laptop", shortcode: "computer" },
      { emoji: "⌨️", name: "keyboard", shortcode: "keyboard" },
      { emoji: "💡", name: "light bulb", shortcode: "bulb" },
      { emoji: "🔦", name: "flashlight", shortcode: "flashlight" },
      { emoji: "🔋", name: "battery", shortcode: "battery" },
      { emoji: "💵", name: "dollar banknote", shortcode: "dollar" },
      { emoji: "💰", name: "money bag", shortcode: "moneybag" },
      { emoji: "💎", name: "gem stone", shortcode: "gem" },
      { emoji: "🔧", name: "wrench", shortcode: "wrench" },
      { emoji: "🔨", name: "hammer", shortcode: "hammer" },
      { emoji: "🛡️", name: "shield", shortcode: "shield" },
      { emoji: "📦", name: "package", shortcode: "package" },
      { emoji: "✉️", name: "envelope", shortcode: "email" },
      { emoji: "📝", name: "memo", shortcode: "memo" },
      { emoji: "📁", name: "file folder", shortcode: "file_folder" },
      { emoji: "📌", name: "pushpin", shortcode: "pushpin" },
      { emoji: "🔒", name: "locked", shortcode: "lock" },
      { emoji: "🔓", name: "unlocked", shortcode: "unlock" },
      { emoji: "🔔", name: "bell", shortcode: "bell" },
      { emoji: "⏱️", name: "stopwatch", shortcode: "stopwatch" },
      { emoji: "⏰", name: "alarm clock", shortcode: "alarm_clock" },
      { emoji: "⏳", name: "hourglass not done", shortcode: "hourglass_flowing_sand" },
      { emoji: "📚", name: "books", shortcode: "books" },
    ] as EmojiItem[],
    symbols: [
      { emoji: "❤️", name: "red heart", shortcode: "heart" },
      { emoji: "🧡", name: "orange heart", shortcode: "orange_heart" },
      { emoji: "💛", name: "yellow heart", shortcode: "yellow_heart" },
      { emoji: "💚", name: "green heart", shortcode: "green_heart" },
      { emoji: "💙", name: "blue heart", shortcode: "blue_heart" },
      { emoji: "💜", name: "purple heart", shortcode: "purple_heart" },
      { emoji: "🖤", name: "black heart", shortcode: "black_heart" },
      { emoji: "🤍", name: "white heart", shortcode: "white_heart" },
      { emoji: "💔", name: "broken heart", shortcode: "broken_heart" },
      { emoji: "💕", name: "two hearts", shortcode: "two_hearts" },
      { emoji: "💖", name: "sparkling heart", shortcode: "sparkling_heart" },
      { emoji: "💯", name: "hundred points", shortcode: "100" },
      { emoji: "💢", name: "anger symbol", shortcode: "anger" },
      { emoji: "💬", name: "speech balloon", shortcode: "speech_balloon" },
      { emoji: "✨", name: "sparkles", shortcode: "sparkles" },
      { emoji: "💥", name: "collision", shortcode: "boom" },
      { emoji: "✅", name: "check mark button", shortcode: "white_check_mark" },
      { emoji: "❌", name: "cross mark", shortcode: "x" },
      { emoji: "⚠️", name: "warning", shortcode: "warning" },
      { emoji: "🟢", name: "green circle", shortcode: "green_circle" },
      { emoji: "🔴", name: "red circle", shortcode: "red_circle" },
    ] as EmojiItem[],
    flags: [
      { emoji: "🏁", name: "chequered flag", shortcode: "checkered_flag" },
      { emoji: "🚩", name: "triangular flag", shortcode: "triangular_flag_on_post" },
      { emoji: "🎌", name: "crossed flags", shortcode: "crossed_flags" },
      { emoji: "🏴", name: "black flag", shortcode: "black_flag" },
      { emoji: "🏳️", name: "white flag", shortcode: "white_flag" },
      { emoji: "🏳️‍🌈", name: "rainbow flag", shortcode: "rainbow_flag" },
      { emoji: "🏴‍☠️", name: "pirate flag", shortcode: "pirate_flag" },
    ] as EmojiItem[],
  } as Record<string, EmojiItem[]>,

  init(textarea: HTMLTextAreaElement, onInsert: (emoji: string) => void): void {
    this.textarea = textarea;
    this.onInsert = onInsert;

    $("btn-emoji")?.addEventListener("click", (e) => {
      e.stopPropagation();
      this.toggle();
    });

    $("btn-close-emoji")?.addEventListener("click", () => this.close());

    $("emoji-search")?.addEventListener("input", (e) => {
      this.searchQuery = (e.target as HTMLInputElement).value.toLowerCase().trim();
      this.render();
    });

    $("emoji-tabs")?.querySelectorAll<HTMLButtonElement>("[data-cat]").forEach((tab) => {
      tab.addEventListener("click", () => {
        $("emoji-tabs")?.querySelectorAll("[data-cat]").forEach((t) => t.classList.remove("btn-active"));
        tab.classList.add("btn-active");
        this.category = tab.getAttribute("data-cat") || "all";
        this.render();
      });
    });

    document.addEventListener("click", (e) => {
      if (
        this.isOpen &&
        !$("composer-emoji-picker")?.contains(e.target as Node) &&
        e.target !== $("btn-emoji")
      ) {
        this.close();
      }
    });

    this.loadOfflineData();
  },

  async loadOfflineData(): Promise<void> {
    try {
      const cached = localStorage.getItem("forest_chat_emojis_cache");
      if (cached) {
        const list = JSON.parse(cached);
        if (Array.isArray(list) && list.length > 0) {
          this.populateFromList(list);
        }
      }
    } catch (_) {}

    try {
      const list = await fetchEmojis();
      if (Array.isArray(list) && list.length > 0) {
        this.populateFromList(list);
        try {
          localStorage.setItem("forest_chat_emojis_cache", JSON.stringify(list));
        } catch (_) {}
      }
    } catch (_) {}
  },

  populateFromList(list: EmojiItem[]): void {
    this.allEmojis = list;
    const grouped: Record<string, EmojiItem[]> = {
      smileys: [],
      people: [],
      nature: [],
      food: [],
      travel: [],
      activities: [],
      objects: [],
      symbols: [],
      flags: [],
    };
    for (const item of list) {
      const g = item.group || "smileys";
      if (!grouped[g]) grouped[g] = [];
      grouped[g].push(item);
    }
    this.data = grouped;
    if (this.isOpen) {
      this.render();
    }
  },

  toggle(): void {
    if (this.isOpen) this.close();
    else this.open();
  },

  open(): void {
    this.isOpen = true;
    $("composer-emoji-picker")?.classList.remove("hidden");
    const search = $("emoji-search") as HTMLInputElement | null;
    if (search) search.value = "";
    this.searchQuery = "";
    this.render();
    setTimeout(() => $("emoji-search")?.focus(), 50);
  },

  close(): void {
    this.isOpen = false;
    $("composer-emoji-picker")?.classList.add("hidden");
  },

  render(): void {
    const grid = $("emoji-grid");
    if (!grid) return;

    let pool: EmojiItem[] = [];
    if (this.category === "all") {
      pool = this.allEmojis.length > 0 ? this.allEmojis : Object.values(this.data).flat();
    } else {
      pool = this.data[this.category] || [];
    }

    if (this.searchQuery) {
      const q = this.searchQuery;
      pool = pool.filter((item) => {
        const em = item.emoji || "";
        const name = (item.name || "").toLowerCase();
        const shortcode = (item.shortcode || "").toLowerCase();
        return em.includes(q) || name.includes(q) || shortcode.includes(q);
      });
    }

    const displayItems = this.searchQuery ? pool.slice(0, 160) : pool.slice(0, 200);

    const items = displayItems.map((item) => {
      const em = item.emoji;
      const name = item.name || "";
      const shortcode = item.shortcode || "";
      const tooltip = name ? `${em} ${name}${shortcode ? ` (:${shortcode}:)` : ""}` : em;

      return h(
        "button",
        {
          type: "button",
          class: "btn btn-ghost btn-xs btn-square text-lg p-0 hover:scale-125 transition-transform",
          title: tooltip,
          onclick: (e: MouseEvent) => {
            e.preventDefault();
            this.insertEmoji(em);
          },
        },
        em
      );
    });

    if (items.length === 0) {
      grid.replaceChildren(
        h(
          "div",
          { class: "col-span-8 py-4 text-center text-xs text-base-content/50" },
          "No emoji found"
        )
      );
    } else {
      grid.replaceChildren(...items);
    }
  },

  insertEmoji(em: string): void {
    if (!this.textarea) return;
    const start = this.textarea.selectionStart;
    const end = this.textarea.selectionEnd;
    const val = this.textarea.value;
    this.textarea.value = val.slice(0, start) + em + val.slice(end);
    const newPos = start + em.length;
    this.textarea.setSelectionRange(newPos, newPos);
    this.textarea.focus();
    this.close();
    if (this.onInsert) {
      this.onInsert(em);
    }
  },
};
