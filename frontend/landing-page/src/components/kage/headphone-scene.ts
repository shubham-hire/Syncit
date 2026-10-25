import type * as ThreeNS from "three";

type Three = typeof ThreeNS;

/**
 * Builds the over-ear headphone + ceramic earbud rig and mounts it in `container`.
 * The rig drifts from hero-right to centre, then separates into a wide
 * headphone/earbud composition as Chapter 4 enters the viewport. Returns a
 * cleanup function.
 *
 * Rendering model: PBR materials (standard/clearcoat) lit by a PMREM-baked
 * gradient environment plus a few directional lights, tone-mapped with ACES.
 * All geometry is procedural — no external model files, so the bundle stays
 * three.js-only.
 */
export function mountHeadphoneScene(container: HTMLElement, THREE: Three): () => void {
  const width = container.clientWidth || window.innerWidth;
  const height = container.clientHeight || window.innerHeight;

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(40, width / height, 0.1, 1000);
  camera.position.set(0, 0, 15);

  const renderer = new THREE.WebGLRenderer({
    alpha: true,
    antialias: true,
    powerPreference: "high-performance",
  });
  renderer.setSize(width, height);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputEncoding = THREE.sRGBEncoding;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.3;
  container.appendChild(renderer.domElement);

  // ---------- Environment (PMREM-baked gradient room with HDR softboxes)
  const envRenderTarget = buildEnvironment(THREE, renderer);
  scene.environment = envRenderTarget.texture;

  // ---------- Lighting (the environment carries the base; these shape form)
  scene.add(new THREE.AmbientLight(0xffffff, 0.42));
  const keyLight = new THREE.DirectionalLight(0xffffff, 1.9);
  keyLight.position.set(10, 14, 12);
  const fillLight = new THREE.DirectionalLight(0xe9eff7, 0.9);
  fillLight.position.set(-12, 4, 10);
  const rimLight = new THREE.DirectionalLight(0xdc3522, 1.35);
  rimLight.position.set(0, -10, -9);
  const topLight = new THREE.DirectionalLight(0xffffff, 1.05);
  topLight.position.set(0, 16, 5);
  scene.add(keyLight, fillLight, rimLight, topLight);

  // ---------- Materials
  const srgb = (hex: number) => new THREE.Color(hex).convertSRGBToLinear();

  const headphoneRedMat = new THREE.MeshPhysicalMaterial({
    color: srgb(0xda2033),
    roughness: 0.36,
    metalness: 0.0,
    clearcoat: 0.55,
    clearcoatRoughness: 0.3,
    envMapIntensity: 0.6,
  });
  const ceramicMat = new THREE.MeshPhysicalMaterial({
    color: srgb(0xe62c3e),
    roughness: 0.24,
    metalness: 0.0,
    clearcoat: 0.9,
    clearcoatRoughness: 0.1,
    envMapIntensity: 0.65,
  });
  const titaniumMat = new THREE.MeshStandardMaterial({
    color: srgb(0xcfd7de),
    metalness: 1.0,
    roughness: 0.28,
  });
  const graphiteMat = new THREE.MeshStandardMaterial({
    color: srgb(0x17191d),
    metalness: 0.6,
    roughness: 0.42,
  });
  const leatherMat = new THREE.MeshStandardMaterial({
    color: srgb(0x5c1622),
    metalness: 0.0,
    roughness: 0.72,
    envMapIntensity: 0.65,
  });
  const grilleMat = new THREE.MeshStandardMaterial({
    color: srgb(0x1c060a),
    metalness: 0.4,
    roughness: 0.55,
  });
  const accentMat = new THREE.MeshStandardMaterial({
    color: srgb(0xe63118),
    metalness: 0.2,
    roughness: 0.3,
  });
  const threadMat = new THREE.MeshStandardMaterial({
    color: srgb(0xe9dccf),
    metalness: 0.0,
    roughness: 0.9,
  });
  const ledMat = new THREE.MeshStandardMaterial({
    color: srgb(0x300402),
    emissive: srgb(0xff3a1e),
    emissiveIntensity: 2.4,
  });

  const masterRig = new THREE.Group();
  scene.add(masterRig);

  /** Lathe helper: `points` are (radius, height) pairs spun around the Y axis. */
  const lathe = (points: Array<[number, number]>, segments = 48) =>
    new THREE.LatheGeometry(
      points.map(([radius, height]) => new THREE.Vector2(radius, height)),
      segments,
    );

  const mesh = (
    parent: ThreeNS.Object3D,
    geometry: ThreeNS.BufferGeometry,
    material: ThreeNS.Material,
    position?: ThreeNS.Vector3Tuple,
  ) => {
    const part = new THREE.Mesh(geometry, material);
    if (position) part.position.set(...position);
    parent.add(part);
    return part;
  };

  // ---------- Headphones
  const headphoneGroup = new THREE.Group();
  masterRig.add(headphoneGroup);

  const headbandCurve = new THREE.CubicBezierCurve3(
    new THREE.Vector3(-3.2, 0.7, 0),
    new THREE.Vector3(-2.5, 4.6, 0),
    new THREE.Vector3(2.5, 4.6, 0),
    new THREE.Vector3(3.2, 0.7, 0),
  );
  mesh(headphoneGroup, new THREE.TubeGeometry(headbandCurve, 72, 0.13, 20, false), headphoneRedMat);

  const strapCurve = new THREE.CubicBezierCurve3(
    new THREE.Vector3(-2.8, 1.1, 0),
    new THREE.Vector3(-2.1, 4.15, 0),
    new THREE.Vector3(2.1, 4.15, 0),
    new THREE.Vector3(2.8, 1.1, 0),
  );
  mesh(headphoneGroup, new THREE.TubeGeometry(strapCurve, 72, 0.3, 20, false), leatherMat);

  // Contrast stitching along the underside of the cushion.
  const stitchGeo = new THREE.BoxGeometry(0.016, 0.016, 0.09);
  const stitchUp = new THREE.Vector3(0, 0, 1);
  const STITCH_COUNT = 12;
  for (let index = 0; index < STITCH_COUNT; index += 1) {
    const t = 0.12 + (index / (STITCH_COUNT - 1)) * 0.76;
    const point = strapCurve.getPointAt(t);
    const tangent = strapCurve.getTangentAt(t).normalize();
    const stitch = new THREE.Mesh(stitchGeo, threadMat);
    stitch.position.set(point.x, point.y - 0.3, point.z);
    stitch.quaternion.setFromUnitVectors(stitchUp, tangent);
    headphoneGroup.add(stitch);
  }

  // Slider housings where the band meets the extendable rods.
  [-2.9, 2.9].forEach((x) => {
    mesh(headphoneGroup, new THREE.CylinderGeometry(0.3, 0.34, 0.72, 24), graphiteMat, [
      x,
      1.05,
      0,
    ]);
    mesh(
      headphoneGroup,
      new THREE.SphereGeometry(0.3, 24, 12, 0, Math.PI * 2, 0, Math.PI / 2),
      graphiteMat,
      [x, 1.41, 0],
    );
    mesh(headphoneGroup, new THREE.CylinderGeometry(0.315, 0.315, 0.14, 24), accentMat, [
      x,
      0.84,
      0,
    ]);
    mesh(headphoneGroup, new THREE.CylinderGeometry(0.07, 0.07, 0.05, 6), titaniumMat, [
      x,
      1.44,
      0,
    ]);
  });

  function createAcousticCup(isLeft: boolean) {
    const side = isLeft ? -1 : 1;
    const cupAssembly = new THREE.Group();

    const sliderRod = mesh(
      cupAssembly,
      new THREE.CylinderGeometry(0.11, 0.11, 1.7, 20),
      titaniumMat,
      [side * 3.25, 0.15, 0],
    );
    // Height-adjustment grooves on the visible part of each rod.
    [0.45, 0.32, 0.19].forEach((y) => {
      const groove = mesh(cupAssembly, new THREE.TorusGeometry(0.115, 0.014, 10, 28), graphiteMat, [
        side * 3.25,
        y,
        0,
      ]);
      groove.rotation.x = Math.PI / 2;
    });

    const yokeGroup = new THREE.Group();
    yokeGroup.position.set(side * 3.28, -0.65, 0);
    const yokeCurve = new THREE.EllipseCurve(0, 0, 1.05, 1.5, -Math.PI / 2, Math.PI / 2, false, 0);
    const yokePoints = yokeCurve
      .getPoints(40)
      .map((p) => new THREE.Vector3(p.x * (isLeft ? 1 : -1) * 0.48, p.y, 0));
    const yoke3DCurve = new THREE.CatmullRomCurve3(yokePoints);
    mesh(yokeGroup, new THREE.TubeGeometry(yoke3DCurve, 40, 0.12, 14, false), graphiteMat);

    [1.5, -1.5].forEach((y) => {
      const hub = mesh(yokeGroup, new THREE.CylinderGeometry(0.15, 0.15, 0.22, 18), titaniumMat, [
        0,
        y,
        0,
      ]);
      hub.rotation.x = Math.PI / 2;
      const pivotScrew = mesh(
        yokeGroup,
        new THREE.CylinderGeometry(0.06, 0.06, 0.05, 6),
        graphiteMat,
        [0, y, side * 0.13],
      );
      pivotScrew.rotation.x = Math.PI / 2;
      const vermDot = mesh(yokeGroup, new THREE.SphereGeometry(0.065, 12, 12), accentMat, [
        0,
        y,
        side * 0.09,
      ]);
      vermDot.position.z = side * 0.1;
    });
    cupAssembly.add(yokeGroup);

    const earcup = new THREE.Group();
    earcup.position.set(side * 3.05, -0.65, 0);
    earcup.rotation.y = side * 0.18;
    earcup.rotation.z = side * 0.05;

    // Barrel shell lathed from a rounded back pole to an open front lip.
    const shell = mesh(
      earcup,
      lathe(
        [
          [0.02, -0.6],
          [0.5, -0.57],
          [0.95, -0.46],
          [1.3, -0.2],
          [1.46, 0.06],
          [1.44, 0.24],
          [1.3, 0.34],
        ],
        56,
      ),
      headphoneRedMat,
    );
    shell.rotation.z = (side * Math.PI) / 2;

    // Titanium rim around the shell opening plus a pinline on the shoulder.
    const rim = mesh(earcup, new THREE.TorusGeometry(1.42, 0.085, 16, 56), titaniumMat, [
      side * 0.3,
      0,
      0,
    ]);
    rim.rotation.y = Math.PI / 2;
    const pinline = mesh(earcup, new THREE.TorusGeometry(1.47, 0.026, 12, 56), accentMat, [
      side * 0.1,
      0,
      0,
    ]);
    pinline.rotation.y = Math.PI / 2;

    // Outer face: recessed driver grille with concentric micro-rings and a
    // centre medallion.
    const grilleDisc = mesh(earcup, new THREE.CylinderGeometry(0.9, 0.9, 0.06, 40), grilleMat, [
      side * 0.5,
      0,
      0,
    ]);
    grilleDisc.rotation.z = Math.PI / 2;
    [0.22, 0.45, 0.68].forEach((radius) => {
      const ring = mesh(earcup, new THREE.TorusGeometry(radius, 0.012, 8, 36), titaniumMat, [
        side * 0.54,
        0,
        0,
      ]);
      ring.rotation.y = Math.PI / 2;
    });
    const medallion = mesh(
      earcup,
      new THREE.CylinderGeometry(0.3, 0.3, 0.12, 32),
      headphoneRedMat,
      [side * 0.55, 0, 0],
    );
    medallion.rotation.z = Math.PI / 2;
    const medallionRing = mesh(earcup, new THREE.TorusGeometry(0.33, 0.02, 10, 32), titaniumMat, [
      side * 0.55,
      0,
      0,
    ]);
    medallionRing.rotation.y = Math.PI / 2;

    // Inner face: baffle plus the oval leather cushion.
    const baffle = mesh(earcup, new THREE.CylinderGeometry(1.0, 1.0, 0.05, 40), grilleMat, [
      -side * 0.3,
      0,
      0,
    ]);
    baffle.rotation.z = Math.PI / 2;
    const cushionGeo = new THREE.TorusGeometry(1.28, 0.42, 26, 44);
    cushionGeo.scale(1, 1.24, 0.9);
    const cushion = new THREE.Mesh(cushionGeo, leatherMat);
    cushion.rotation.y = Math.PI / 2;
    cushion.position.x = -side * 0.4;
    earcup.add(cushion);

    // Power indicator on the right cup rim.
    if (!isLeft) {
      mesh(earcup, new THREE.SphereGeometry(0.05, 12, 12), ledMat, [side * 0.3, -1.36, 0.25]);
    }

    cupAssembly.add(earcup);
    return cupAssembly;
  }

  headphoneGroup.add(createAcousticCup(true), createAcousticCup(false));

  // ---------- Ceramic earbuds
  const earbudsGroup = new THREE.Group();
  masterRig.add(earbudsGroup);

  function createCeramicEarbud(isLeft: boolean) {
    const bud = new THREE.Group();
    const side = isLeft ? -1 : 1;

    // Teardrop body lathed around Y, squashed on Z for the in-ear profile.
    const body = mesh(
      bud,
      lathe(
        [
          [0.02, 0.6],
          [0.3, 0.5],
          [0.48, 0.24],
          [0.54, -0.02],
          [0.5, -0.28],
          [0.4, -0.5],
          [0.24, -0.6],
          [0.02, -0.62],
        ],
        40,
      ),
      ceramicMat,
    );
    body.scale.z = 0.88;

    // Angled nozzle with metal collar, mesh, and a soft silicone tip.
    const nozzle = new THREE.Group();
    nozzle.rotation.x = 0.52;
    nozzle.rotation.z = side * 0.65;
    nozzle.position.set(side * 0.3, 0.32, 0.4);
    const nozzleBody = mesh(
      nozzle,
      new THREE.CylinderGeometry(0.13, 0.16, 0.18, 24),
      graphiteMat,
      [0, 0.02, 0],
    );
    nozzleBody.rotation.x = 0;
    const collar = mesh(
      nozzle,
      new THREE.TorusGeometry(0.135, 0.03, 10, 24),
      titaniumMat,
      [0, -0.06, 0],
    );
    collar.rotation.x = Math.PI / 2;
    const tipFlange = mesh(
      nozzle,
      new THREE.TorusGeometry(0.125, 0.05, 12, 24),
      leatherMat,
      [0, 0.14, 0],
    );
    tipFlange.rotation.x = Math.PI / 2;
    mesh(
      nozzle,
      new THREE.SphereGeometry(0.13, 20, 14, 0, Math.PI * 2, 0, Math.PI / 2),
      leatherMat,
      [0, 0.16, 0],
    );
    bud.add(nozzle);

    // IR wear sensor on the inner face near the nozzle base.
    const irSensor = mesh(bud, new THREE.SphereGeometry(0.032, 12, 10), graphiteMat, [
      side * 0.42,
      0.05,
      0.26,
    ]);
    irSensor.scale.set(1, 1.4, 0.5);

    // Tapered stem with a rounded bottom and a microphone port.
    const stem = mesh(
      bud,
      lathe(
        [
          [0.125, 0.1],
          [0.12, -0.5],
          [0.105, -0.85],
          [0.06, -0.95],
          [0.02, -0.97],
        ],
        32,
      ),
      ceramicMat,
      [side * 0.1, -0.75, -0.02],
    );
    stem.rotation.z = side * -0.12;
    const baseRing = mesh(bud, new THREE.TorusGeometry(0.1, 0.018, 10, 24), titaniumMat, [
      side * 0.115,
      -1.58,
      -0.03,
    ]);
    baseRing.rotation.x = Math.PI / 2;
    const micPort = mesh(bud, new THREE.SphereGeometry(0.028, 10, 8), graphiteMat, [
      side * 0.2,
      -1.28,
      0.08,
    ]);
    micPort.scale.set(0.5, 1.7, 0.5);

    // Touch control dot and the floating shell ring accent.
    mesh(bud, new THREE.SphereGeometry(0.055, 12, 12), accentMat, [side * 0.36, -0.3, 0.3]);
    const shellRing = mesh(
      bud,
      new THREE.TorusGeometry(0.44, 0.03, 12, 36),
      headphoneRedMat,
      [0, 0.05, 0.42],
    );
    shellRing.rotation.x = 0.15;

    return bud;
  }

  const earbudL = createCeramicEarbud(true);
  earbudL.position.set(-1.8, -2.3, 1.5);
  earbudL.rotation.set(0.28, 0.55, -0.18);
  const earbudR = createCeramicEarbud(false);
  earbudR.position.set(1.8, -2.2, 1.4);
  earbudR.rotation.set(-0.2, -0.65, 0.22);
  earbudsGroup.add(earbudL, earbudR);

  const TARGET_SCALE = 0.82;
  masterRig.scale.set(TARGET_SCALE, TARGET_SCALE, TARGET_SCALE);

  // Keep the Chapter 4 split in camera space. The master rig rotates while the
  // page scrolls, so applying a raw local x offset would sometimes send the
  // products diagonally or to the wrong side of the screen.
  const inverseMasterRotation = new THREE.Quaternion();
  const worldOffset = new THREE.Vector3();
  let chapterSplit = 0;

  // ---------- Interaction
  let mouseX = 0;
  let mouseY = 0;
  let targetRotX = 0;
  let targetRotY = 0;

  const onMouseMove = (e: MouseEvent) => {
    mouseX = ((e.clientX / window.innerWidth) * 2 - 1) * 0.32;
    mouseY = (-(e.clientY / window.innerHeight) * 2 + 1) * 0.2;
  };
  window.addEventListener("mousemove", onMouseMove);

  function getScrollProgress() {
    const scrollTop = window.pageYOffset || document.documentElement.scrollTop;
    let effectiveMaxScroll = document.documentElement.scrollHeight - window.innerHeight;
    if (footer) {
      const footerTop = footer.getBoundingClientRect().top + scrollTop;
      effectiveMaxScroll = Math.max(footerTop - window.innerHeight - 80, 100);
    }
    return Math.min(Math.max(scrollTop / effectiveMaxScroll, 0), 1);
  }

  /**
   * Hero presence while the hero section is on screen; eases to 0 as the hero
   * tail leaves the viewport. On wide screens a floor opacity keeps the rig
   * alive as a quiet floating layer between the hero and the finale.
   */
  function getHeroVisibility() {
    const hero = document.getElementById("hero");
    if (!hero) return 0;
    const heroBottom = hero.getBoundingClientRect().bottom;
    const fadeStart = window.innerHeight * 0.98;
    const fadeEnd = window.innerHeight * 0.8;
    return Math.min(Math.max((heroBottom - fadeEnd) / (fadeStart - fadeEnd), 0), 1);
  }

  function getChapterProgress(id: string) {
    const section = document.getElementById(id);
    if (!section) return 0;

    const sectionTop = section.getBoundingClientRect().top;
    const start = window.innerHeight * 0.92;
    const end = window.innerHeight * 0.24;
    return Math.min(Math.max((start - sectionTop) / (start - end), 0), 1);
  }

  function smoothstep(value: number) {
    return value * value * (3 - 2 * value);
  }

  const clock = new THREE.Clock();
  const footer = document.querySelector<HTMLElement>("footer");
  let frame = 0;
  let canvasOpacity = -1;

  // Finale pose: centred at full size with the exploded split.
  const FINALE_Y = -0.35;

  const heroVisual = document.querySelector<HTMLElement>(".hero-visual");

  /**
   * Anchor the hero pose to the .hero-visual column, so the product sits
   * exactly where the layout reserved space for it — beside the copy on wide
   * screens, under it on narrow ones — at any viewport size. Rect coordinates
   * are converted to world units at the camera's depth (z=15, fov 40).
   */
  function heroPose() {
    const visibleHeight = 2 * Math.tan((40 * Math.PI) / 180 / 2) * 15;
    const visibleWidth = visibleHeight * (window.innerWidth / window.innerHeight);
    if (!heroVisual) {
      return { x: 0, y: -4.2, scale: 0.62 };
    }
    const rect = heroVisual.getBoundingClientRect();
    // Once the column scrolls out of view, hold the anchor on-screen; the
    // scroll drift takes over from there.
    const cx = Math.min(Math.max(rect.left + rect.width / 2, 0), window.innerWidth);
    const cy = Math.min(
      Math.max(rect.top + rect.height / 2, window.innerHeight * 0.3),
      window.innerHeight * 0.85,
    );
    const x = ((cx / window.innerWidth) * 2 - 1) * (visibleWidth / 2);
    const y = -((cy / window.innerHeight) * 2 - 1) * (visibleHeight / 2);
    const scale = Math.min(Math.max(rect.height / (visibleHeight * 0.82), 0.5), 1.02);
    return { x, y, scale };
  }

  function animate() {
    frame = requestAnimationFrame(animate);
    const delta = Math.min(clock.getDelta(), 0.05);
    const t = clock.getElapsedTime();
    const scrollP = getScrollProgress();
    const chapterSplitTarget = getChapterProgress("eternity");
    const splitDamping = 1 - Math.exp(-delta * 8);

    chapterSplit += (chapterSplitTarget - chapterSplit) * splitDamping;
    const splitP = smoothstep(chapterSplit);
    const heroV = getHeroVisibility();

    targetRotY += (mouseX - targetRotY) * 0.05;
    targetRotX += (mouseY - targetRotX) * 0.05;

    // Retire the rig as the footer rises into view: lift clear of the footer
    // edge and dissolve, so the products never slide underneath the footer.
    let exitFactor = 0;
    if (footer) {
      const footerRise = window.innerHeight - footer.getBoundingClientRect().top;
      exitFactor = Math.min(Math.max(footerRise / (window.innerHeight * 0.38), 0), 1);
    }
    const exit = smoothstep(exitFactor);

    // Wide screens keep the product floating through the page: it detaches
    // from its hero column as you scroll, shrinks to a quiet presence that
    // passes behind the frosted cards, and returns centre stage for the
    // finale split. Narrow screens fade it out between hero and finale so it
    // never sits behind full-width text.
    const floating = window.innerWidth > 1024;
    const drift = smoothstep(Math.min(scrollP / 0.18, 1));
    const gateP = smoothstep(getChapterProgress("gate"));
    const hero = heroPose();
    const intro = Math.min(t / 1.2, 1);
    const fromRight = (1 - intro) * 1.6;

    const posX = (hero.x + (1.35 - hero.x) * drift) * (1 - splitP);
    let posY = hero.y + (-0.2 - hero.y) * drift;
    posY += (FINALE_Y - posY) * splitP + Math.sin(t * 1.2) * 0.05 + exit * 1.7;
    let scale = hero.scale + (0.55 - hero.scale) * drift;
    scale += (TARGET_SCALE - scale) * splitP;

    masterRig.position.x = posX + (floating ? fromRight : 0);
    masterRig.position.y = posY;
    masterRig.scale.set(scale, scale, scale);
    masterRig.rotation.y = 0.3 + scrollP * 2.2 + targetRotY + splitP * 0.35;
    masterRig.rotation.x = -0.05 + targetRotX * 0.6 + Math.sin(scrollP * Math.PI) * 0.08;

    // Separate in screen space so the headphones always travel right and the
    // earbuds always travel left, regardless of the rig's rotation.
    inverseMasterRotation.copy(masterRig.quaternion).invert();
    const splitDistance = window.innerWidth <= 600 ? 1.55 : window.innerWidth <= 1024 ? 3.6 : 6.2;

    worldOffset
      .set(splitDistance * splitP, 0.2 * splitP, 0)
      .applyQuaternion(inverseMasterRotation)
      .multiplyScalar(1 / scale);
    headphoneGroup.position.copy(worldOffset);
    headphoneGroup.rotation.z = splitP * 0.06;

    worldOffset
      .set(-splitDistance * 0.9 * splitP, -0.16 * splitP, 0)
      .applyQuaternion(inverseMasterRotation)
      .multiplyScalar(1 / scale);
    earbudsGroup.position.copy(worldOffset);
    earbudsGroup.rotation.z = -splitP * 0.08;

    earbudL.position.y = -2.3 + Math.sin(t * 1.4 + 1.2) * 0.08;
    earbudR.position.y = -2.2 + Math.cos(t * 1.3 + 2.0) * 0.08;

    // Soft intro on load; in the hero the product is fully present, while
    // floating it settles to a faint watermark — visible in motion, quiet at
    // rest, and dipped further while #gate's wide content is on screen.
    const cruiseOpacity = floating ? Math.max(0.26 - gateP * 0.08, 0.18) : 0;
    const presence = Math.max(heroV, cruiseOpacity, splitP * 0.94);
    const targetOpacity = presence * intro * (1 - exit);
    if (Math.abs(targetOpacity - canvasOpacity) > 0.004) {
      canvasOpacity = targetOpacity;
      renderer.domElement.style.opacity = targetOpacity.toFixed(3);
    }
    if (targetOpacity > 0.001) {
      renderer.render(scene, camera);
    }
  }
  animate();

  const onResize = () => {
    const w = container.clientWidth || window.innerWidth;
    const h = container.clientHeight || window.innerHeight;
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    renderer.setSize(w, h);
  };
  window.addEventListener("resize", onResize);

  return () => {
    cancelAnimationFrame(frame);
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("resize", onResize);
    envRenderTarget.dispose();
    scene.traverse((obj) => {
      const asMesh = obj as ThreeNS.Mesh;
      if (asMesh.geometry) asMesh.geometry.dispose();
    });
    [
      headphoneRedMat,
      ceramicMat,
      titaniumMat,
      graphiteMat,
      leatherMat,
      grilleMat,
      accentMat,
      threadMat,
      ledMat,
    ].forEach((m) => m.dispose());
    renderer.dispose();
    renderer.domElement.remove();
  };
}

/**
 * Bakes a small gradient room with three HDR softboxes into a PMREM texture so
 * the clearcoat and metal materials have something believable to reflect.
 */
function buildEnvironment(
  THREE: Three,
  renderer: ThreeNS.WebGLRenderer,
): ThreeNS.WebGLRenderTarget {
  const pmrem = new THREE.PMREMGenerator(renderer);
  const envScene = new THREE.Scene();

  const sky = new THREE.Mesh(
    new THREE.SphereGeometry(60, 32, 20),
    new THREE.ShaderMaterial({
      side: THREE.BackSide,
      uniforms: {
        topColor: { value: new THREE.Color(0xffffff) },
        midColor: { value: new THREE.Color(0xdfe7f0) },
        bottomColor: { value: new THREE.Color(0xb9c3cd) },
      },
      vertexShader: `
        varying vec3 vDir;
        void main() {
          vDir = normalize(position);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        varying vec3 vDir;
        uniform vec3 topColor;
        uniform vec3 midColor;
        uniform vec3 bottomColor;
        void main() {
          float h = clamp(vDir.y * 0.5 + 0.5, 0.0, 1.0);
          vec3 c = h > 0.5
            ? mix(midColor, topColor, h * 2.0 - 1.0)
            : mix(bottomColor, midColor, h * 2.0);
          gl_FragColor = vec4(c, 1.0);
        }
      `,
    }),
  );
  envScene.add(sky);

  const softbox = (x: number, y: number, z: number, w: number, h: number, intensity: number) => {
    const panel = new THREE.Mesh(
      new THREE.PlaneGeometry(w, h),
      new THREE.ShaderMaterial({
        uniforms: { uIntensity: { value: intensity } },
        vertexShader: `
          void main() {
            gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
          }
        `,
        fragmentShader: `
          uniform float uIntensity;
          void main() {
            gl_FragColor = vec4(vec3(uIntensity), 1.0);
          }
        `,
        side: THREE.DoubleSide,
      }),
    );
    panel.position.set(x, y, z);
    panel.lookAt(0, 0, 0);
    envScene.add(panel);
  };

  softbox(-18, 14, 16, 22, 14, 3.4); // key softbox, upper left front
  softbox(20, 6, 10, 16, 10, 2.1); // fill, right
  softbox(0, -16, -14, 26, 12, 1.0); // dim bounce from below

  const renderTarget = pmrem.fromScene(envScene, 0.045);
  pmrem.dispose();
  envScene.traverse((obj) => {
    const asMesh = obj as ThreeNS.Mesh;
    if (asMesh.geometry) asMesh.geometry.dispose();
    if (asMesh.material) (asMesh.material as ThreeNS.Material).dispose();
  });
  return renderTarget;
}
