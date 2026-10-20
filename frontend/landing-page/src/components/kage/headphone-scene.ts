import type * as ThreeNS from "three";

type Three = typeof ThreeNS;

/**
 * Builds the over-ear headphone + ceramic earbud rig and mounts it in `container`.
 * The rig drifts from hero-right to centre, then separates into a wide
 * headphone/earbud composition as Chapter 4 enters the viewport. Returns a
 * cleanup function.
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
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  container.appendChild(renderer.domElement);

  // Lighting
  scene.add(new THREE.AmbientLight(0xffffff, 1.8));
  const keyLight = new THREE.DirectionalLight(0xffffff, 3.2);
  keyLight.position.set(12, 16, 14);
  const fillLight = new THREE.DirectionalLight(0xedf2f7, 2.0);
  fillLight.position.set(-14, 6, 10);
  const rimLight = new THREE.DirectionalLight(0xdc3522, 2.4);
  rimLight.position.set(0, -12, -8);
  const topLight = new THREE.DirectionalLight(0xffffff, 2.5);
  topLight.position.set(0, 18, 4);
  scene.add(keyLight, fillLight, rimLight, topLight);

  // Materials
  const graphiteMat = new THREE.MeshPhongMaterial({
    color: 0x111316,
    specular: 0x99a4b2,
    shininess: 120,
  });
  const headphoneRedMat = new THREE.MeshPhongMaterial({
    color: 0xb51f2b,
    specular: 0xffd4d4,
    shininess: 180,
  });
  const earbudRedMat = new THREE.MeshPhongMaterial({
    color: 0xd92c3a,
    specular: 0xffffff,
    shininess: 210,
  });
  const brushedTitaniumMat = new THREE.MeshPhongMaterial({
    color: 0xc8d0d8,
    specular: 0xffffff,
    shininess: 150,
  });
  const leatherCushionMat = new THREE.MeshLambertMaterial({ color: 0x340b12 });
  const grilleMeshMat = new THREE.MeshPhongMaterial({
    color: 0x26070d,
    specular: 0x9b3540,
    shininess: 80,
  });
  const vermilionAccentMat = new THREE.MeshPhongMaterial({
    color: 0xee2c18,
    emissive: 0x4a0701,
    specular: 0xff8877,
    shininess: 130,
  });

  const masterRig = new THREE.Group();
  scene.add(masterRig);

  // ---------- Headphones
  const headphoneGroup = new THREE.Group();
  masterRig.add(headphoneGroup);

  const headbandCurve = new THREE.CubicBezierCurve3(
    new THREE.Vector3(-3.2, 0.7, 0),
    new THREE.Vector3(-2.5, 4.6, 0),
    new THREE.Vector3(2.5, 4.6, 0),
    new THREE.Vector3(3.2, 0.7, 0),
  );
  headphoneGroup.add(
    new THREE.Mesh(new THREE.TubeGeometry(headbandCurve, 64, 0.13, 16, false), headphoneRedMat),
  );

  const strapCurve = new THREE.CubicBezierCurve3(
    new THREE.Vector3(-2.8, 1.1, 0),
    new THREE.Vector3(-2.1, 4.15, 0),
    new THREE.Vector3(2.1, 4.15, 0),
    new THREE.Vector3(2.8, 1.1, 0),
  );
  headphoneGroup.add(
    new THREE.Mesh(new THREE.TubeGeometry(strapCurve, 64, 0.3, 16, false), leatherCushionMat),
  );

  [-2.9, 2.9].forEach((x) => {
    const clamp = new THREE.Mesh(new THREE.BoxGeometry(0.32, 0.6, 0.48), brushedTitaniumMat);
    clamp.position.set(x, 1.1, 0);
    const redAccent = new THREE.Mesh(
      new THREE.CylinderGeometry(0.08, 0.08, 0.5, 16),
      vermilionAccentMat,
    );
    redAccent.rotation.z = Math.PI / 2;
    redAccent.position.set(x, 1.1, 0);
    headphoneGroup.add(clamp, redAccent);
  });

  function createAcousticCup(isLeft: boolean) {
    const side = isLeft ? -1 : 1;
    const cupAssembly = new THREE.Group();

    const sliderRod = new THREE.Mesh(
      new THREE.CylinderGeometry(0.11, 0.11, 1.7, 20),
      brushedTitaniumMat,
    );
    sliderRod.position.set(side * 3.25, 0.15, 0);
    cupAssembly.add(sliderRod);

    const yokeGroup = new THREE.Group();
    yokeGroup.position.set(side * 3.28, -0.65, 0);
    const yokeCurve = new THREE.EllipseCurve(0, 0, 1.05, 1.5, -Math.PI / 2, Math.PI / 2, false, 0);
    const yokePoints = yokeCurve
      .getPoints(40)
      .map((p) => new THREE.Vector3(p.x * (isLeft ? 1 : -1) * 0.48, p.y, 0));
    const yoke3DCurve = new THREE.CatmullRomCurve3(yokePoints);
    yokeGroup.add(
      new THREE.Mesh(new THREE.TubeGeometry(yoke3DCurve, 40, 0.12, 14, false), graphiteMat),
    );

    [1.5, -1.5].forEach((y) => {
      const hub = new THREE.Mesh(
        new THREE.CylinderGeometry(0.15, 0.15, 0.22, 18),
        brushedTitaniumMat,
      );
      hub.rotation.x = Math.PI / 2;
      hub.position.set(0, y, 0);
      const vermDot = new THREE.Mesh(new THREE.SphereGeometry(0.065, 12, 12), vermilionAccentMat);
      vermDot.position.set(0, y, side * 0.12);
      yokeGroup.add(hub, vermDot);
    });
    cupAssembly.add(yokeGroup);

    const earcup = new THREE.Group();
    earcup.position.set(side * 3.05, -0.65, 0);
    earcup.rotation.y = side * 0.18;
    earcup.rotation.z = side * 0.05;

    const cupOuter = new THREE.Mesh(
      new THREE.CylinderGeometry(1.5, 1.58, 0.74, 44),
      headphoneRedMat,
    );
    cupOuter.rotation.z = Math.PI / 2;
    earcup.add(cupOuter);

    const titaniumRim = new THREE.Mesh(
      new THREE.CylinderGeometry(1.46, 1.46, 0.15, 44),
      brushedTitaniumMat,
    );
    titaniumRim.rotation.z = Math.PI / 2;
    titaniumRim.position.x = side * 0.38;
    earcup.add(titaniumRim);

    const vermilionRing = new THREE.Mesh(
      new THREE.TorusGeometry(1.42, 0.045, 16, 50),
      vermilionAccentMat,
    );
    vermilionRing.rotation.y = Math.PI / 2;
    vermilionRing.position.x = side * 0.27;
    earcup.add(vermilionRing);

    const planarGrille = new THREE.Mesh(
      new THREE.CylinderGeometry(0.98, 0.98, 0.14, 38),
      grilleMeshMat,
    );
    planarGrille.rotation.z = Math.PI / 2;
    planarGrille.position.x = side * 0.44;
    earcup.add(planarGrille);

    const centerMedallion = new THREE.Mesh(
      new THREE.CylinderGeometry(0.4, 0.4, 0.18, 28),
      headphoneRedMat,
    );
    centerMedallion.rotation.z = Math.PI / 2;
    centerMedallion.position.x = side * 0.46;
    earcup.add(centerMedallion);

    const cushionGeo = new THREE.TorusGeometry(1.3, 0.46, 26, 44);
    cushionGeo.scale(1, 1.3, 0.88);
    const cushion = new THREE.Mesh(cushionGeo, leatherCushionMat);
    cushion.rotation.y = Math.PI / 2;
    cushion.position.x = -side * 0.4;
    earcup.add(cushion);

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

    const capsuleGeo = new THREE.SphereGeometry(0.54, 32, 24);
    capsuleGeo.scale(1, 1.25, 0.9);
    bud.add(new THREE.Mesh(capsuleGeo, earbudRedMat));

    const nozzle = new THREE.Mesh(new THREE.CylinderGeometry(0.19, 0.23, 0.42, 20), graphiteMat);
    nozzle.rotation.x = 0.52;
    nozzle.rotation.z = side * 0.65;
    nozzle.position.set(side * 0.34, 0.23, 0.36);
    bud.add(nozzle);

    const tip = new THREE.Mesh(new THREE.SphereGeometry(0.27, 20, 16), leatherCushionMat);
    tip.scale.set(1, 0.84, 1.3);
    tip.position.set(side * 0.5, 0.36, 0.62);
    bud.add(tip);

    const stem = new THREE.Mesh(new THREE.CylinderGeometry(0.13, 0.11, 1.6, 24), earbudRedMat);
    stem.position.set(side * 0.11, -0.9, -0.05);
    stem.rotation.z = side * -0.12;
    bud.add(stem);

    const baseCap = new THREE.Mesh(
      new THREE.CylinderGeometry(0.12, 0.12, 0.17, 20),
      brushedTitaniumMat,
    );
    baseCap.position.set(side * 0.21, -1.7, -0.05);
    bud.add(baseCap);

    const touchDot = new THREE.Mesh(new THREE.SphereGeometry(0.06, 12, 12), vermilionAccentMat);
    touchDot.position.set(side * 0.24, -0.56, 0.09);
    bud.add(touchDot);

    const shellRing = new THREE.Mesh(new THREE.TorusGeometry(0.42, 0.035, 12, 28), headphoneRedMat);
    shellRing.position.set(0, 0.02, 0.49);
    bud.add(shellRing);

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
    const footer = document.querySelector("footer");
    let effectiveMaxScroll = document.documentElement.scrollHeight - window.innerHeight;
    if (footer) {
      const footerTop = footer.getBoundingClientRect().top + scrollTop;
      effectiveMaxScroll = Math.max(footerTop - window.innerHeight - 80, 100);
    }
    return Math.min(Math.max(scrollTop / effectiveMaxScroll, 0), 1);
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

  // On narrow screens the hero stacks, so keep the rig centred.
  const heroOffsetX = () => (window.innerWidth <= 1024 ? 0 : 2.6);

  const clock = new THREE.Clock();
  let frame = 0;

  function animate() {
    frame = requestAnimationFrame(animate);
    const delta = Math.min(clock.getDelta(), 0.05);
    const t = clock.getElapsedTime();
    const scrollP = getScrollProgress();
    const chapterSplitTarget = getChapterProgress("eternity");
    const splitDamping = 1 - Math.exp(-delta * 8);

    chapterSplit += (chapterSplitTarget - chapterSplit) * splitDamping;
    const splitP = smoothstep(chapterSplit);

    targetRotY += (mouseX - targetRotY) * 0.05;
    targetRotX += (mouseY - targetRotX) * 0.05;

    const centerFactor = Math.min(scrollP / 0.32, 1);
    const posX = (1 - centerFactor) * heroOffsetX();
    const posY = (1 - centerFactor) * 0.35 + centerFactor * -0.1;

    masterRig.position.x = posX;
    masterRig.position.y = posY + Math.sin(t * 1.2) * 0.05;
    masterRig.rotation.y = scrollP * Math.PI * 2.2 + targetRotY;
    masterRig.rotation.x = Math.sin(scrollP * Math.PI) * 0.16 + targetRotX * 0.6;

    // Separate in screen space so the headphones always travel right and the
    // earbuds always travel left, regardless of the rig's scroll rotation.
    inverseMasterRotation.copy(masterRig.quaternion).invert();
    const splitDistance = window.innerWidth <= 600 ? 1.55 : window.innerWidth <= 1024 ? 3.6 : 6.2;

    worldOffset
      .set(splitDistance * splitP, 0.2 * splitP, 0)
      .applyQuaternion(inverseMasterRotation)
      .multiplyScalar(1 / TARGET_SCALE);
    headphoneGroup.position.copy(worldOffset);
    headphoneGroup.rotation.z = splitP * 0.06;

    worldOffset
      .set(-splitDistance * 0.9 * splitP, -0.16 * splitP, 0)
      .applyQuaternion(inverseMasterRotation)
      .multiplyScalar(1 / TARGET_SCALE);
    earbudsGroup.position.copy(worldOffset);
    earbudsGroup.rotation.z = -splitP * 0.08;

    earbudL.position.y = -2.3 + Math.sin(t * 1.4 + 1.2) * 0.08;
    earbudR.position.y = -2.2 + Math.cos(t * 1.3 + 2.0) * 0.08;

    renderer.render(scene, camera);
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
    scene.traverse((obj) => {
      const mesh = obj as ThreeNS.Mesh;
      if (mesh.geometry) mesh.geometry.dispose();
    });
    [
      graphiteMat,
      brushedTitaniumMat,
      leatherCushionMat,
      grilleMeshMat,
      vermilionAccentMat,
      headphoneRedMat,
      earbudRedMat,
    ].forEach((m) => m.dispose());
    renderer.dispose();
    renderer.domElement.remove();
  };
}
